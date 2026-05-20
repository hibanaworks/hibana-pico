import CryptoKit
import Foundation

public enum ApprovalAction: String, Codable, Equatable, Sendable {
    case nod
    case reject
    case fence
}

public struct ApprovalRequest: Codable, Equatable, Sendable {
    public var txId: UInt64
    public var generation: UInt32
    public var workspaceId: UInt64
    public var objectId: UInt64
    public var bodyHash: String
    public var summaryHash: String
    public var nonce: UInt64

    public init(
        txId: UInt64,
        generation: UInt32,
        workspaceId: UInt64,
        objectId: UInt64,
        bodyHash: String,
        summaryHash: String,
        nonce: UInt64
    ) {
        self.txId = txId
        self.generation = generation
        self.workspaceId = workspaceId
        self.objectId = objectId
        self.bodyHash = bodyHash
        self.summaryHash = summaryHash
        self.nonce = nonce
    }
}

public struct ApprovalEvidence: Codable, Equatable, Sendable {
    public var request: ApprovalRequest
    public var action: ApprovalAction
    public var displayedHash: String
    public var publicKey: String
    public var signature: String

    public init(
        request: ApprovalRequest,
        action: ApprovalAction,
        displayedHash: String,
        publicKey: String,
        signature: String
    ) {
        self.request = request
        self.action = action
        self.displayedHash = displayedHash
        self.publicKey = publicKey
        self.signature = signature
    }
}

public struct DisplayedIntent: Equatable, Sendable {
    public var request: ApprovalRequest
    public var displayedHash: String
}

public enum PicoNodAppError: Error, Equatable {
    case malformedHex
    case signatureRejected
}

public struct PicoNodApprovalApp: Sendable {
    private let privateKey: Curve25519.Signing.PrivateKey

    public init(privateKey: Curve25519.Signing.PrivateKey) {
        self.privateKey = privateKey
    }

    public static func fresh() -> Self {
        Self(privateKey: Curve25519.Signing.PrivateKey())
    }

    public var publicKeyHex: String {
        privateKey.publicKey.rawRepresentation.hexString
    }

    public func display(_ request: ApprovalRequest) -> DisplayedIntent {
        DisplayedIntent(request: request, displayedHash: Self.displayedHash(request))
    }

    public func decide(_ displayed: DisplayedIntent, action: ApprovalAction) throws -> ApprovalEvidence {
        let payload = Self.signingPayload(displayed.request, action: action, displayedHash: displayed.displayedHash)
        let signature = try privateKey.signature(for: payload)
        return ApprovalEvidence(
            request: displayed.request,
            action: action,
            displayedHash: displayed.displayedHash,
            publicKey: publicKeyHex,
            signature: signature.hexString
        )
    }

    public static func verify(_ evidence: ApprovalEvidence) throws -> Bool {
        guard
            let publicKeyBytes = Data(hexString: evidence.publicKey),
            let signatureBytes = Data(hexString: evidence.signature)
        else {
            throw PicoNodAppError.malformedHex
        }
        let publicKey = try Curve25519.Signing.PublicKey(rawRepresentation: publicKeyBytes)
        let payload = signingPayload(
            evidence.request,
            action: evidence.action,
            displayedHash: evidence.displayedHash
        )
        return publicKey.isValidSignature(signatureBytes, for: payload)
    }

    public static func displayedHash(_ request: ApprovalRequest) -> String {
        let payload = canonicalBytes(request)
        return Data(SHA256.hash(data: payload)).hexString
    }

    private static func signingPayload(
        _ request: ApprovalRequest,
        action: ApprovalAction,
        displayedHash: String
    ) -> Data {
        var payload = canonicalBytes(request)
        payload.append(0x0a)
        payload.append(action.rawValue.data(using: .utf8)!)
        payload.append(0x0a)
        payload.append(displayedHash.data(using: .utf8)!)
        return payload
    }

    private static func canonicalBytes(_ request: ApprovalRequest) -> Data {
        let text = [
            "tx_id=\(request.txId)",
            "generation=\(request.generation)",
            "workspace_id=\(request.workspaceId)",
            "object_id=\(request.objectId)",
            "body_hash=\(request.bodyHash)",
            "summary_hash=\(request.summaryHash)",
            "nonce=\(request.nonce)",
        ].joined(separator: "\n")
        return text.data(using: .utf8)!
    }
}

extension Data {
    fileprivate init?(hexString: String) {
        guard hexString.count.isMultiple(of: 2) else {
            return nil
        }
        var bytes = [UInt8]()
        bytes.reserveCapacity(hexString.count / 2)
        var index = hexString.startIndex
        while index < hexString.endIndex {
            let next = hexString.index(index, offsetBy: 2)
            guard let byte = UInt8(hexString[index..<next], radix: 16) else {
                return nil
            }
            bytes.append(byte)
            index = next
        }
        self = Data(bytes)
    }

    fileprivate var hexString: String {
        map { String(format: "%02x", $0) }.joined()
    }
}
