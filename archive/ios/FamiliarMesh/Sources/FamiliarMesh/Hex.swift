import Foundation

/// Lower-case hex, no `0x`, no separators — the encoding every byte field on the mesh wire uses
/// (node/group ids, pubkeys, signatures). Matches the Rust `hex_encode`/`hex_decode`.
public enum Hex {
    public static func encode<S: Sequence>(_ bytes: S) -> String where S.Element == UInt8 {
        var s = ""
        s.reserveCapacity(bytes.underestimatedCount * 2)
        for b in bytes {
            s.append(Character(UnicodeScalar(hexDigit(b >> 4))))
            s.append(Character(UnicodeScalar(hexDigit(b & 0xf))))
        }
        return s
    }

    public static func decode(_ s: String) -> Data? {
        let chars = Array(s.utf8)
        guard chars.count % 2 == 0 else { return nil }
        var out = Data(capacity: chars.count / 2)
        var i = 0
        while i < chars.count {
            guard let hi = nibble(chars[i]), let lo = nibble(chars[i + 1]) else { return nil }
            out.append(hi << 4 | lo)
            i += 2
        }
        return out
    }

    private static func hexDigit(_ n: UInt8) -> UInt8 {
        n < 10 ? (0x30 + n) : (0x61 + (n - 10)) // '0'..'9' then 'a'..'f'
    }

    private static func nibble(_ c: UInt8) -> UInt8? {
        switch c {
        case 0x30...0x39: return c - 0x30
        case 0x61...0x66: return c - 0x61 + 10
        case 0x41...0x46: return c - 0x41 + 10
        default: return nil
        }
    }
}
