import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))  # mock_backend, mock_<area>
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
