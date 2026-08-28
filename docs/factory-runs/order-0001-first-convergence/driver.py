"""SP548E BLE device driver for state query and decode."""


def build_state_query() -> bytes:
    """
    Build a state-query frame for SP548E device.
    
    Frame format: 0x53|type|key|total_frags|frag_idx|payload_len|payload
    - type: 0x02 (state query)
    - key: 0x00 (unencrypted)
    - total_frags: 0x01 (single fragment)
    - frag_idx: 0x00 (first/only fragment)
    - payload_len: 0x00 (empty payload)
    
    Returns:
        bytes: The complete state query frame.
    """
    frame = bytearray()
    frame.append(0x53)      # frame header
    frame.append(0x02)      # type: state query
    frame.append(0x00)      # key: unencrypted
    frame.append(0x01)      # total_frags: single fragment
    frame.append(0x00)      # frag_idx: first fragment
    frame.append(0x00)      # payload_len: empty
    # no payload
    return bytes(frame)


def decode_state(reply: bytes) -> dict:
    """
    Decode state query response from SP548E device.
    
    Extracts mode and brightness from the state response payload.
    According to protocol specification:
    - byte[30]: mode
    - byte[33]: brightness
    
    Args:
        reply: Raw bytes received from device state query response.
        
    Returns:
        dict: Dictionary with 'mode' and 'brightness' keys (bytes).
              Returns None values if reply is too short.
    """
    result = {
        'mode': None,
        'brightness': None
    }
    
    # Minimum payload to access byte[33]
    if len(reply) >= 34:
        result['mode'] = reply[30]
        result['brightness'] = reply[33]
    
    return result
