from enum import Enum
from dataclasses import dataclass, is_dataclass
from copy import deepcopy

import exchange_types

FRACTIONAL_BITS = 31  # I33F31 -> 31 fractional bits
PRICE_SCALE = 2 ** FRACTIONAL_BITS

# Encodes the price as a fixed point integer
def encoded_price(f: float):
    return [round(f * PRICE_SCALE)]

# We define a generic Command type from which each specific Command type 
# (or really Command struct) will inherit
class Command():
    def encode(self):
        fields_as_dict = deepcopy(self.__dict__)
        # Encode fields named `price`
        if 'price' in fields_as_dict:
            fields_as_dict['price'] = encoded_price(fields_as_dict['price'])

        # Encode enums as just the underlying value and (data)classes as a tuple
        # of their fields
        for k, v in fields_as_dict.items():
            if isinstance(v, Enum):
                fields_as_dict[k] = v.value
            elif is_dataclass(v):
                fields_as_dict[k] = list(v.__dict__.values())

        fields_as_struct = list(fields_as_dict.values())
        return {
            f"{type(self).__name__}": fields_as_struct
        }

# We define a generic CommandResult type from which each specific CommandResult
# type will inherit
class CommandResult():
    pass

@dataclass
class OrderInsert(Command):
    # The id of the account that created the order.
    account_id: AccountId
    # The type of the order (limit, market, etc.). Field is `order_type` because `type` is a reserved keyword.
    order_type: OrderType
    # The pair that the order should be executed on.
    pair: AssetIdPair
    # Side of the order (Bid / Ask).
    side: Side
    # Volume of the order in whole units.
    volume: Volume
    # Price of the order.
    price: Price

@dataclass
class OrderCancel(Command):
    # The id of the account that created the order.
    account_id: AccountId
    # The id of the order to cancel
    order_id: OrderId

@dataclass
class OrderModify(Command):
    # The id of the account that created the order.
    account_id: AccountId
    # The id of the order to modify
    order_id: OrderId
    # The new volume for the order. Must be lower than original.
    new_volume: Volume

@dataclass
class GetBalance(Command):
    # The id of the account that created the order.
    account_id: AccountId
    # The id of the order to modify
    asset_id: AssetId

@dataclass
class GetOrderBookL1(Command):
    pair: AssetIdPair

@dataclass
class GetOrderBookL2(Command):
    pair: AssetIdPair

@dataclass
class GetAssets(Command):
    pass

@dataclass
class GetAllOrderbookL1(Command):
    pass
