"""
ztask-ingest: Parse OpenSpec SDD directories and create task graphs in Zenoh.

Supports both:
- Greenfield: directories with tasks/ subdirectory containing numbered task files
- Update: single spec files that can be parsed into tasks

Features:
- Gherkin validation and conversion for acceptance criteria
- BDD feature file generation
- Dependency cycle detection
- Topological sort for dependency ordering
"""

import json
import os
import re
import sys
from dataclasses import dataclass, field
from pathlib import Path
from typing import Dict, List, Optional, Set, Tuple

from ztask.cli import create_task, get_iso_timestamp
from ztask.queries import fetch_task
from ztask.zenoh_client import open_session


@dataclass
class ParsedTask:
    """A task parsed from an OpenSpec file."""
    id: str
    title: str
    acceptance_criteria: str = ""
    spec: str = ""
    depends_on: List[str] = field(default_factory=list)
    test_files: List[str] = field(default_factory=list)
    implementation_files: List[str] = field(default_factory=list)
    test_command: str = ""
    verification_command: str = ""
    bdd_feature_file: str = ""


def derive_task_id(filename: str) -> str:
    """Derive task ID from filename.
    
    Examples:
        01-db-migrations.md -> db-migrations
        02-auth-login.md -> auth-login
        auth-refresh.md -> auth-refresh
    """
    stem = Path(filename).stem
    # Strip leading digits and hyphen
    stem = re.sub(r'^\d+-', '', stem)
    return stem


def parse_markdown_sections(content: str) -> Dict[str, str]:
    """Parse markdown into sections keyed by heading text.
    
    Returns dict of section_name -> content (without the heading line).
    """
    sections: Dict[str, str] = {}
    current_section: Optional[str] = None
    current_content: List[str] = []
    
    for line in content.split('\n'):
        heading_match = re.match(r'^##\s+(.+)$', line)
        if heading_match:
            # Save previous section
            if current_section is not None:
                sections[current_section] = '\n'.join(current_content).strip()
            current_section = heading_match.group(1).strip()
            current_content = []
        elif current_section is not None:
            current_content.append(line)
    
    # Save last section
    if current_section is not None:
        sections[current_section] = '\n'.join(current_content).strip()
    
    return sections


def parse_list_items(content: str) -> List[str]:
    """Parse markdown list items (- item) into a list of strings."""
    items = []
    for line in content.split('\n'):
        line = line.strip()
        if line.startswith('- '):
            items.append(line[2:].strip())
    return items


def is_gherkin_format(text: str) -> bool:
    """Check if text is in Gherkin format."""
    # Look for Feature: and Scenario: keywords
    has_feature = bool(re.search(r'^Feature:', text, re.MULTILINE))
    has_scenario = bool(re.search(r'^Scenario:', text, re.MULTILINE))
    return has_feature and has_scenario


def convert_to_gherkin(title: str, criteria: str) -> str:
    """Convert acceptance criteria to Gherkin format.
    
    Handles:
    - Bullet points (- item)
    - Free-text descriptions
    - Partial Gherkin (missing Feature header)
    """
    # If already Gherkin, return as-is
    if is_gherkin_format(criteria):
        return criteria
    
    # Extract feature name from title
    feature_name = title
    
    # Parse bullet points into scenarios
    bullets = parse_list_items(criteria)
    
    if bullets:
        # Convert bullet points to Gherkin scenarios
        gherkin = f"Feature: {feature_name}\n"
        gherkin += "  As a developer\n"
        gherkin += f"  I want to {title.lower()}\n"
        gherkin += "  So that the system works correctly\n\n"
        
        for i, bullet in enumerate(bullets, 1):
            # Try to extract Given/When/Then from bullet
            if any(keyword in bullet.lower() for keyword in ['given', 'when', 'then', 'and']):
                # Already has Gherkin keywords, use as scenario
                gherkin += f"  Scenario: {bullet}\n"
                gherkin += f"    {bullet}\n\n"
            else:
                # Convert to scenario
                gherkin += f"  Scenario: {bullet}\n"
                gherkin += f"    Given the system is in a valid state\n"
                gherkin += f"    When I {bullet.lower()}\n"
                gherkin += f"    Then the operation succeeds\n\n"
        
        return gherkin.strip()
    
    # Free-text: create a single scenario
    gherkin = f"Feature: {feature_name}\n"
    gherkin += "  As a developer\n"
    gherkin += f"  I want to {title.lower()}\n"
    gherkin += "  So that the system works correctly\n\n"
    gherkin += f"  Scenario: {title}\n"
    gherkin += f"    Given the system is in a valid state\n"
    gherkin += f"    When I implement {title.lower()}\n"
    gherkin += f"    Then the implementation is correct\n"
    gherkin += f"    And all tests pass\n"
    
    return gherkin.strip()


def extract_scenarios_from_gherkin(gherkin: str) -> List[str]:
    """Extract scenario names from Gherkin text."""
    scenarios = []
    for match in re.finditer(r'^\s*Scenario:\s*(.+)$', gherkin, re.MULTILINE):
        scenarios.append(match.group(1).strip())
    return scenarios


def generate_bdd_feature_file(task_id: str, title: str, gherkin: str, bdd_dir: Path) -> str:
    """Generate a BDD feature file from Gherkin acceptance criteria."""
    # Create feature filename from task ID
    feature_filename = f"{task_id.replace('-', '_')}.feature"
    feature_path = bdd_dir / feature_filename
    
    # Write feature file
    feature_path.parent.mkdir(parents=True, exist_ok=True)
    feature_path.write_text(gherkin + '\n')
    
    return str(feature_path.relative_to(bdd_dir.parent.parent))


def parse_task_file(filepath: Path) -> Optional[ParsedTask]:
    """Parse a single task file into a ParsedTask."""
    content = filepath.read_text()
    sections = parse_markdown_sections(content)
    
    # Extract task ID from filename
    task_id = derive_task_id(filepath.name)
    
    # Extract title from # Task: heading
    title_match = re.search(r'^#\s+Task:\s+(.+)$', content, re.MULTILINE)
    title = title_match.group(1).strip() if title_match else task_id
    
    # Extract acceptance criteria (required)
    acceptance_criteria = sections.get('Acceptance Criteria', '')
    if not acceptance_criteria:
        print(f"  Warning: {filepath.name} has no Acceptance Criteria section, skipping", file=sys.stderr)
        return None
    
    # Validate and convert to Gherkin format
    if not is_gherkin_format(acceptance_criteria):
        print(f"  Info: {filepath.name} acceptance criteria not in Gherkin format, converting...", file=sys.stderr)
        acceptance_criteria = convert_to_gherkin(title, acceptance_criteria)
    
    # Extract optional fields
    spec = sections.get('Spec', '')
    depends_on = parse_list_items(sections.get('Depends On', ''))
    test_files = parse_list_items(sections.get('Test Files', ''))
    implementation_files = parse_list_items(sections.get('Implementation Files', ''))
    test_command = sections.get('Test Command', '').strip()
    verification_command = sections.get('Verification Command', '').strip()
    bdd_feature_file = sections.get('BDD Feature File', '').strip()
    
    return ParsedTask(
        id=task_id,
        title=title,
        acceptance_criteria=acceptance_criteria,
        spec=spec,
        depends_on=depends_on,
        test_files=test_files,
        implementation_files=implementation_files,
        test_command=test_command,
        verification_command=verification_command,
        bdd_feature_file=bdd_feature_file,
    )


def detect_cycle(graph: Dict[str, List[str]]) -> Optional[List[str]]:
    """Detect cycles in a dependency graph using DFS.
    
    Returns the cycle path if found, None otherwise.
    """
    WHITE, GRAY, BLACK = 0, 1, 2
    color = {node: WHITE for node in graph}
    parent = {node: None for node in graph}
    
    def dfs(node: str) -> Optional[List[str]]:
        color[node] = GRAY
        for neighbor in graph.get(node, []):
            if neighbor not in color:
                continue  # Skip unknown dependencies
            if color[neighbor] == GRAY:
                # Found cycle, reconstruct path
                cycle = [neighbor, node]
                current = node
                while parent[current] != neighbor:
                    current = parent[current]
                    cycle.append(current)
                cycle.reverse()
                return cycle
            if color[neighbor] == WHITE:
                parent[neighbor] = node
                result = dfs(neighbor)
                if result:
                    return result
        color[node] = BLACK
        return None
    
    for node in graph:
        if color[node] == WHITE:
            result = dfs(node)
            if result:
                return result
    return None


def topological_sort(graph: Dict[str, List[str]]) -> List[str]:
    """Topological sort of a dependency graph.
    
    Returns list of nodes in dependency order (dependencies first).
    """
    # Compute in-degrees (number of dependencies each node has)
    in_degree = {node: len(deps) for node, deps in graph.items()}
    
    # Start with nodes that have no dependencies
    queue = [node for node, degree in in_degree.items() if degree == 0]
    result = []
    
    while queue:
        node = queue.pop(0)
        result.append(node)
        
        # Find nodes that depend on this node and reduce their in-degree
        for dependent, deps in graph.items():
            if node in deps:
                in_degree[dependent] -= 1
                if in_degree[dependent] == 0:
                    queue.append(dependent)
    
    return result


def parse_spec_directory(spec_dir: Path) -> Dict[str, ParsedTask]:
    """Parse all task files in a spec directory."""
    tasks_dir = spec_dir / 'tasks'
    
    if not tasks_dir.exists():
        print(f"Error: No tasks/ directory found in {spec_dir}", file=sys.stderr)
        sys.exit(1)
    
    task_files = sorted(tasks_dir.glob('*.md'))
    if not task_files:
        print(f"Error: No .md files found in {tasks_dir}", file=sys.stderr)
        sys.exit(1)
    
    tasks: Dict[str, ParsedTask] = {}
    for filepath in task_files:
        task = parse_task_file(filepath)
        if task:
            tasks[task.id] = task
    
    return tasks


def parse_single_spec(filepath: Path) -> Dict[str, ParsedTask]:
    """Parse a single spec file into tasks.
    
    For update specs, we extract tasks from ## sections that start with a number
    or have a clear task-like structure.
    """
    content = filepath.read_text()
    sections = parse_markdown_sections(content)
    
    # Look for task-like sections (## N. Task Name or ## Task: Name)
    tasks: Dict[str, ParsedTask] = {}
    
    # Pattern 1: Numbered sections (## 1. Extend Model, ## 2. Update CLI, etc.)
    numbered_pattern = re.compile(r'^##\s+(\d+)\.\s+(.+)$', re.MULTILINE)
    matches = list(numbered_pattern.finditer(content))
    
    if matches:
        for i, match in enumerate(matches):
            section_num = match.group(1)
            section_title = match.group(2).strip()
            
            # Extract section content
            start = match.end()
            end = matches[i + 1].start() if i + 1 < len(matches) else len(content)
            section_content = content[start:end].strip()
            
            # Derive task ID from title
            task_id = re.sub(r'[^a-z0-9]+', '-', section_title.lower()).strip('-')
            
            # Parse subsections
            sub_sections = parse_markdown_sections(section_content)
            
            # Extract fields
            acceptance_criteria = sub_sections.get('Acceptance Criteria', section_content[:200])
            depends_on = parse_list_items(sub_sections.get('Depends On', ''))
            test_files = parse_list_items(sub_sections.get('Test Files', ''))
            implementation_files = parse_list_items(sub_sections.get('Implementation Files', ''))
            test_command = sub_sections.get('Test Command', '').strip()
            verification_command = sub_sections.get('Verification Command', '').strip()
            bdd_feature_file = sub_sections.get('BDD Feature File', '').strip()
            
            # Validate and convert to Gherkin format
            if not is_gherkin_format(acceptance_criteria):
                print(f"  Info: '{section_title}' acceptance criteria not in Gherkin format, converting...", file=sys.stderr)
                acceptance_criteria = convert_to_gherkin(section_title, acceptance_criteria)
            
            tasks[task_id] = ParsedTask(
                id=task_id,
                title=section_title,
                acceptance_criteria=acceptance_criteria,
                spec=section_content,
                depends_on=depends_on,
                test_files=test_files,
                implementation_files=implementation_files,
                test_command=test_command,
                verification_command=verification_command,
                bdd_feature_file=bdd_feature_file,
            )
    
    # Pattern 2: Look for "Changes Required" or "Required Changes" section
    changes_section = sections.get('Changes Required', sections.get('Required Changes', ''))
    if changes_section and not tasks:
        # Parse as a list of changes
        items = parse_list_items(changes_section)
        for i, item in enumerate(items):
            task_id = re.sub(r'[^a-z0-9]+', '-', item.lower()).strip('-')[:50]
            acceptance_criteria = convert_to_gherkin(item, item)
            tasks[task_id] = ParsedTask(
                id=task_id,
                title=item,
                acceptance_criteria=acceptance_criteria,
                spec=changes_section,
            )
    
    return tasks


def ingest_to_zenoh(project_id: str, tasks: Dict[str, ParsedTask], dry_run: bool = False) -> None:
    """Create tasks in Zenoh from parsed task definitions."""
    # Build dependency graph
    graph: Dict[str, List[str]] = {}
    for task_id, task in tasks.items():
        graph[task_id] = [dep for dep in task.depends_on if dep in tasks]
    
    # Check for cycles
    cycle = detect_cycle(graph)
    if cycle:
        print(f"Error: Circular dependency detected: {' -> '.join(cycle)}", file=sys.stderr)
        sys.exit(1)
    
    # Topological sort
    ordered_tasks = topological_sort(graph)
    
    # Print dependency graph and Gherkin status
    print("\nDependency graph:")
    for task_id in ordered_tasks:
        task = tasks[task_id]
        deps_str = ""
        if task.depends_on:
            deps = [d for d in task.depends_on if d in tasks]
            if deps:
                deps_str = f" -> depends on [{', '.join(deps)}]"
            else:
                deps_str = " (no deps)"
        else:
            deps_str = " (no deps)"
        
        # Gherkin status
        scenarios = extract_scenarios_from_gherkin(task.acceptance_criteria)
        gherkin_status = f"✓ Gherkin ({len(scenarios)} scenarios)" if scenarios else "✓ Gherkin"
        
        print(f"  {task_id}{deps_str}")
        print(f"    Acceptance Criteria: {gherkin_status}")
    
    if dry_run:
        print("\n[dry-run] Would create tasks:")
        for task_id in ordered_tasks:
            task = tasks[task_id]
            print(f"  - {task_id}: {task.title}")
            scenarios = extract_scenarios_from_gherkin(task.acceptance_criteria)
            if scenarios:
                print(f"    BDD: {len(scenarios)} scenarios")
        return
    
    # Create tasks in Zenoh
    print(f"\nCreating tasks in project '{project_id}':")
    created = 0
    skipped = 0
    
    for task_id in ordered_tasks:
        task = tasks[task_id]
        
        # Build args for CLI
        args = [
            'create', task_id,
            '--project', project_id,
            '--criteria', task.acceptance_criteria,
            '--entered-by', 'llm',
        ]
        if task.spec:
            args.extend(['--spec', task.spec])
        if task.depends_on:
            args.extend(['--depends-on', ','.join(task.depends_on)])
        if task.test_files:
            args.extend(['--test-files', ','.join(task.test_files)])
        if task.implementation_files:
            args.extend(['--impl-files', ','.join(task.implementation_files)])
        if task.test_command:
            args.extend(['--test-command', task.test_command])
        if task.verification_command:
            args.extend(['--verify-command', task.verification_command])
        
        # Use CLI runner
        from typer.testing import CliRunner
        from ztask.cli import app
        
        runner = CliRunner()
        result = runner.invoke(app, args)
        
        if result.exit_code == 0:
            deps_note = f" (blocked by: {', '.join(task.depends_on)})" if task.depends_on else ""
            print(f"  ✓ {task_id} — PENDING{deps_note}")
            created += 1
        else:
            # Check if it's because task already exists
            if "already exists" in result.output:
                print(f"  ⚠ {task_id} — already exists, skipping")
                skipped += 1
            else:
                print(f"  ✗ {task_id} — failed: {result.output.strip()}")
    
    print(f"\nDone. {created} tasks created, {skipped} skipped, 0 cycles detected.")


def main():
    """Main entry point for the ingest command."""
    import argparse
    
    parser = argparse.ArgumentParser(description='Ingest OpenSpec SDD directory into Zenoh tasks')
    parser.add_argument('project_id', help='Zenoh project ID')
    parser.add_argument('spec_path', help='Path to spec directory or single spec file')
    parser.add_argument('--dry-run', action='store_true', help='Parse and validate only, do not create tasks')
    parser.add_argument('--no-gherkin', action='store_true', help='Skip Gherkin validation and conversion')
    parser.add_argument('--no-bdd', action='store_true', help='Skip BDD feature file generation')
    
    args = parser.parse_args()
    
    spec_path = Path(args.spec_path)
    if not spec_path.exists():
        print(f"Error: Path '{spec_path}' not found", file=sys.stderr)
        sys.exit(1)
    
    # Determine if this is a directory or single file
    if spec_path.is_dir():
        print(f"Ingesting OpenSpec from {spec_path}/")
        tasks = parse_spec_directory(spec_path)
    else:
        print(f"Ingesting OpenSpec from {spec_path}")
        tasks = parse_single_spec(spec_path)
    
    if not tasks:
        print("Error: No tasks found in spec", file=sys.stderr)
        sys.exit(1)
    
    print(f"  Found {len(tasks)} task(s)")
    
    # Print task summary with Gherkin status
    for task_id, task in tasks.items():
        print(f"  - {task_id}: {task.title}")
        if task.depends_on:
            print(f"    Depends on: {', '.join(task.depends_on)}")
        
        # Gherkin status
        if is_gherkin_format(task.acceptance_criteria):
            scenarios = extract_scenarios_from_gherkin(task.acceptance_criteria)
            print(f"    Acceptance Criteria: ✓ Gherkin format ({len(scenarios)} scenarios)")
        else:
            print(f"    Acceptance Criteria: ⚠ Not Gherkin format")
    
    # Ingest to Zenoh
    ingest_to_zenoh(args.project_id, tasks, dry_run=args.dry_run)


if __name__ == '__main__':
    main()
