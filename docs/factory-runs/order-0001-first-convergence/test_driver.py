"""Self-test for SP548E driver."""

import sys
from driver import build_state_query, decode_state


def test_build_state_query():
    """Test state query frame construction."""
    frame = build_state_query()
    
    # Verify frame structure
    assert isinstance(frame, bytes), "build_state_query must return bytes"
    assert len(frame) == 6, f"Frame length should be 6, got {len(frame)}"
    
    # Verify frame bytes
    assert frame[0] == 0x53, f"Header should be 0x53, got {hex(frame[0])}"
    assert frame[1] == 0x02, f"Type should be 0x02, got {hex(frame[1])}"
    assert frame[2] == 0x00, f"Key should be 0x00, got {hex(frame[2])}"
    assert frame[3] == 0x01, f"Total frags should be 0x01, got {hex(frame[3])}"
    assert frame[4] == 0x00, f"Frag idx should be 0x00, got {hex(frame[4])}"
    assert frame[5] == 0x00, f"Payload len should be 0x00, got {hex(frame[5])}"
    
    print("✓ build_state_query frame structure verified")


def test_decode_state():
    """Test state response decoding."""
    # Create a minimal fixture with 34 bytes (sufficient for byte[33])
    reply = bytearray(34)
    reply[30] = 0x42  # mode = 0x42
    reply[33] = 0xFF  # brightness = 0xFF (max)
    
    result = decode_state(bytes(reply))
    
    assert isinstance(result, dict), "decode_state must return dict"
    assert 'mode' in result, "Result must have 'mode' key"
    assert 'brightness' in result, "Result must have 'brightness' key"
    assert result['mode'] == 0x42, f"Mode should be 0x42, got {hex(result['mode'])}"
    assert result['brightness'] == 0xFF, f"Brightness should be 0xFF, got {hex(result['brightness'])}"
    
    print("✓ decode_state with full payload verified")


def test_decode_state_short():
    """Test decode with insufficient payload."""
    # Short reply (less than 34 bytes)
    reply = bytearray(20)
    result = decode_state(bytes(reply))
    
    assert result['mode'] is None, "Mode should be None for short reply"
    assert result['brightness'] is None, "Brightness should be None for short reply"
    
    print("✓ decode_state gracefully handles short payload")


if __name__ == '__main__':
    try:
        test_build_state_query()
        test_decode_state()
        test_decode_state_short()
        print("\n✓ All tests passed")
        sys.exit(0)
    except AssertionError as e:
        print(f"\n✗ Test failed: {e}", file=sys.stderr)
        sys.exit(1)
