from dataclasses import dataclass, field
from typing import List, Optional


@dataclass
class Task:
    id: str
    status: str = "UNKNOWN"
    time_entered: Optional[str] = None
    time_accepted: Optional[str] = None
    time_completed: Optional[str] = None
    acceptance_criteria: Optional[str] = None
    entered_by: Optional[str] = None
    history: List[dict] = field(default_factory=list)

    # SDD fields
    spec: Optional[str] = None
    depends_on: List[str] = field(default_factory=list)
    blocks: List[str] = field(default_factory=list)

    # TDD fields
    test_files: List[str] = field(default_factory=list)
    implementation_files: List[str] = field(default_factory=list)
    tdd_phase: Optional[str] = None
    test_command: Optional[str] = None
    verification_command: Optional[str] = None

    # Execution metadata
    failure_reason: Optional[str] = None
    attempt_count: int = 0

    def to_dict(self) -> dict:
        d = {
            "id": self.id,
            "status": self.status,
            "time_entered": self.time_entered,
            "time_accepted": self.time_accepted,
            "time_completed": self.time_completed,
            "acceptance_criteria": self.acceptance_criteria,
            "entered_by": self.entered_by,
            "history": self.history,
        }
        # SDD fields
        if self.spec is not None:
            d["spec"] = self.spec
        if self.depends_on:
            d["depends_on"] = self.depends_on
        if self.blocks:
            d["blocks"] = self.blocks
        # TDD fields
        if self.test_files:
            d["test_files"] = self.test_files
        if self.implementation_files:
            d["implementation_files"] = self.implementation_files
        if self.tdd_phase is not None:
            d["tdd_phase"] = self.tdd_phase
        if self.test_command is not None:
            d["test_command"] = self.test_command
        if self.verification_command is not None:
            d["verification_command"] = self.verification_command
        # Execution metadata
        if self.failure_reason is not None:
            d["failure_reason"] = self.failure_reason
        if self.attempt_count > 0:
            d["attempt_count"] = self.attempt_count
        return d
