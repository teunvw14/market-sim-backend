# MessagePack Framing

The exchange server receives and sends raw bytes; but it needs `Command` objects for processing. Each (buffer of) `Command`(s) is encoded with MessagePack (the `rmp` crate specifically). The process of converting from raw bytes to chunks of bytes representing `CommandBuffer`s is called "framing". Here we use a very simple algorithm (if one can even call it that): each frame is prepended with two bytes denoting the length of the frame that is being sent.

# Framing Example

Say we want to send two frames (`Command`s) which are 2 and 5 bytes long respectively. The bytes sent would be:

[0x00, 0x01, 0xab, 0xcd, 0x00, 0x05, 0x12, 0x34, 0x56, 0x78, 0x9a]
|----------  ----------  ---------|  ----------------------------
|             Command 1           |            Command 2
|                                 |
|-- "Next Command 2 bytes long"   |
                                  |
Next Command 3 bytes long --------|

(In practice the `Command` structure is larger of course.)

# Framing Errors

Framing can go wrong in a few ways:

1. The prepended length is too large. This is not a concern in our case since 2 bytes already limits the size to 256 * 256 = ~64KB.
2. Length incorrectly computed by sender, or not included at all. We handle this by disconnecting the sender, since this is an indication that the sender has not correctly implemented the framing rules, meaning that subsequent received bytes will also likely not be framed correctly.
