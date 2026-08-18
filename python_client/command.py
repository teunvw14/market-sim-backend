from enum import Enum
from dataclasses import dataclass, is_dataclass
from copy import deepcopy
from typing import get_args, get_origin, Iterable

from python_client.exchange_types import AccountId, AssetId, AssetIdPair, OrderId, OrderType, Side, Volume, Price, Result, Ok, Err, OrderExecutionStatus
from python_client.exchange_errors import OrderInsertionError, OrderCancellationError, OrderModificationError, MarketCreationError, GetOrderbookError

FRACTIONAL_BITS = 31  # I33F31 -> 31 fractional bits
PRICE_SCALE = 2 ** FRACTIONAL_BITS

# Encodes the price as a fixed point integer
def encoded_price(f: float):
    return [round(f * PRICE_SCALE)]

# Encodes the price as a fixed point integer
def decoded_price(f: list[int]):
    return f[0] / PRICE_SCALE

# Helper types for (de)serializing Option and Result types

def class_with_fields(t, value):
    '''
    Tries to create an instance of t with `value` as the fields
    '''
    if value is None:
        return None
    if isinstance(value, Iterable):
        return t(*value)
    else:
        return t(value)

class ResultLike():
    okType = type
    errType = type

    @classmethod
    def _decode(cls, obj):
        if isinstance(obj, Ok):
            return Ok(class_with_fields(cls.okType, obj.value))
        elif isinstance(obj, Err):
            return Err(class_with_fields(cls.errType, obj.value))

# We define a generic Command type from which each specific Command type 
# (or really Command struct) will inherit
@dataclass
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
        # Reduce (or unnest) if there is only one field
        if len(fields_as_struct) == 1:
            fields_as_struct = fields_as_struct[0]
        return {
            f"{type(self).__name__}": fields_as_struct
        }

# We follow this general pattern:
# Define the command (with the same name as in the source code). 
# CommandDecoder is a class which must provide a `_decode` function.
# Then finally there is a type / class which represents the final object 
# returned to the client API user.

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
class OrderInsertionEffects():
    id: int
    status: OrderExecutionStatus
    
class OrderInsertionDecoder(ResultLike):
    okType = OrderInsertionEffects
    errType = OrderInsertionError

OrderInsertionResult = Ok[OrderInsertionDecoder.okType] | Err[OrderInsertionDecoder.errType]

@dataclass
class OrderCancel(Command):
    # The id of the account that created the order.
    account_id: AccountId
    # The id of the order to cancel
    order_id: OrderId

class OrderCancellationDecoder(ResultLike):
    okType = None
    errType = OrderCancellationError

OrderCancellationResult = Ok[OrderCancellationDecoder.okType] | Err[OrderCancellationDecoder.errType]

@dataclass
class OrderModify(Command):
    # The id of the account that created the order.
    account_id: AccountId
    # The id of the order to modify
    order_id: OrderId
    # The new volume for the order. Must be lower than original.
    new_volume: Volume

class OrderModificationDecoder(ResultLike):
    okType = None
    errType = OrderModificationError

OrderModificationResult = Ok[OrderModificationDecoder.okType] | Err[OrderModificationDecoder.errType]

@dataclass
class GetBalance(Command):
    # The id of the account that created the order.
    account_id: AccountId
    # The id of the order to modify
    asset_id: AssetId

@dataclass
class GetBalanceResponse():
    @staticmethod
    def _decode(obj):
        if obj is None:
            return None
        else:
            return decoded_price(obj)

@dataclass
class GetOrderbookL1(Command):
    pair: AssetIdPair

@dataclass
class PriceLevelAggregate:
    price: float
    volume: int

@dataclass
class OrderbookL1():
    best_bid: PriceLevelAggregate | None
    best_ask: PriceLevelAggregate | None

class GetOrderbookL1Response(ResultLike):
    okType = OrderbookL1
    errType = GetOrderbookError

@dataclass
class GetOrderbookL2(Command):
    pair: AssetIdPair

@dataclass
class GetAssets(Command):
    pass

@dataclass
class GetAllOrderbookL1(Command):
    pass


# Commands with a Result as a response
COMMAND_RESULT_TYPES = {
    'OrderInsert': OrderInsertionDecoder,
    'OrderCancel': OrderCancellationDecoder,
    'OrderModify': OrderModificationDecoder,
    'GetBalance': GetBalanceResponse,
    'GetOrderbookL1': GetOrderbookL1Response,
}

def decode_commands(obj):
    print(f'decoding object {obj}')
    if obj is None:
        return None
    for k, t in COMMAND_RESULT_TYPES.items():
        if k in obj:
            return t._decode(obj[k])
    if 'Ok' in obj:
        return Ok(obj['Ok'])
    elif 'Err' in obj:
        return Err(obj['Err'])
    return obj
