import os

def diff(old: str, new: str) -> str:
    """Diff two Typst documents given as source strings, returning diff markup."""

def diff_files(old_path: str | os.PathLike[str], new_path: str | os.PathLike[str]) -> str:
    """Diff two Typst documents given as file paths, returning diff markup."""
