from python_client.client import ExchangeClient, OrderInsert
from python_client.exchange_types import OrderType, Side, AssetIdPair

client = ExchangeClient("127.0.0.1")

result = client.send_command(OrderInsert(10, OrderType.Limit, AssetIdPair(1, 0), Side.Ask, 100, 1))

print(result)
