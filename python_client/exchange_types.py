from dataclasses import dataclass
from enum import Enum

# Types
class OrderType(Enum):
    Limit = 0
    FillOrKill = 1
    Market = 2

class Side(Enum):
    Bid = 0
    Ask = 1

AssetId = int
AccountId = int
Volume = int
Price = int
OrderId = int

@dataclass
class Asset:
    id: AssetId
    name: str
    symbol: str

@dataclass
class AssetPair():
    primary: Asset
    secondary: Asset

@dataclass
class AssetIdPair:
    primary: AssetId
    secondary: AssetId
