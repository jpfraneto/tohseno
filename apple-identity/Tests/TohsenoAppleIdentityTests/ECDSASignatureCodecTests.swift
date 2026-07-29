import Foundation
import Testing
@testable import TohsenoAppleIdentity

@Test
func parsesAndReencodesMinimalDER() throws {
    let der = Data([0x30, 0x06, 0x02, 0x01, 0x01, 0x02, 0x01, 0x02])
    let components = try ECDSASignatureCodec.fixedWidthComponents(fromDER: der)
    #expect(components.r == Data(repeating: 0, count: 31) + Data([1]))
    #expect(components.s == Data(repeating: 0, count: 31) + Data([2]))
    #expect(try ECDSASignatureCodec.derSignature(from: components) == der)
}

@Test
func encodesRequiredPositiveIntegerPadding() throws {
    let r = Data([0x80]) + Data(repeating: 0, count: 31)
    let s = Data(repeating: 0, count: 31) + Data([1])
    let components = try P256SignatureComponents(r: r, s: s)
    let der = try ECDSASignatureCodec.derSignature(from: components)
    #expect(der.prefix(5) == Data([0x30, 0x26, 0x02, 0x21, 0x00]))
    #expect(try ECDSASignatureCodec.fixedWidthComponents(fromDER: der) == components)
}

@Test
func normalizesHighSAgainstTheP256Order() throws {
    #expect(ECDSASignatureCodec.p256Order.hexadecimal(prefix: false)
        == "ffffffff00000000ffffffffffffffffbce6faada7179e84f3b9cac2fc632551")
    #expect(ECDSASignatureCodec.p256HalfOrder.hexadecimal(prefix: false)
        == "7fffffff800000007fffffffffffffffde737d56d38bcf4279dce5617e3192a8")
    var highS = Array(ECDSASignatureCodec.p256Order)
    highS[31] -= 1
    let normalized = try ECDSASignatureCodec.lowS(Data(highS))
    #expect(normalized == Data(repeating: 0, count: 31) + Data([1]))
    #expect(ECDSASignatureCodec.isLowS(normalized))
}

@Test(arguments: [
    Data([0x30, 0x06, 0x02, 0x01, 0x80, 0x02, 0x01, 0x01]),
    Data([0x30, 0x07, 0x02, 0x02, 0x00, 0x01, 0x02, 0x01, 0x01]),
    Data([0x30, 0x06, 0x02, 0x01, 0x00, 0x02, 0x01, 0x01]),
])
func rejectsNegativeNonminimalAndZeroScalars(_ der: Data) {
    #expect(throws: (any Error).self) {
        try ECDSASignatureCodec.fixedWidthComponents(fromDER: der)
    }
}
