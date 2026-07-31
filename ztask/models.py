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
    history: List[dict] = field(default_factory=list)

    def to_dict(self) -> dict:
        return {
            "id": self.id,
            "status": self.status,
            "time_entered": self.time_entered,
            "time_accepted": self.time_accepted,
            "time_completed": self.time_completed,
            "acceptance_criteria": self.acceptance_criteria,
            "history": self.history,
        }
