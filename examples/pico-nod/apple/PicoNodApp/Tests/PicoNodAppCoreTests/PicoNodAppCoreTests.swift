import PicoNodAppCore
import Testing

@Test
func nodEvidenceVerifiesForDisplayedIntent() throws {
    let app = PicoNodApprovalApp.fresh()
    let request = ApprovalRequest(
        txId: 11,
        generation: 1,
        workspaceId: 3,
        objectId: 44,
        bodyHash: "body",
        summaryHash: "summary",
        nonce: 99
    )
    let displayed = app.display(request)
    let evidence = try app.decide(displayed, action: .nod)

    #expect(evidence.displayedHash == displayed.displayedHash)
    #expect(evidence.action == .nod)
    #expect(try PicoNodApprovalApp.verify(evidence))
}

@Test
func tamperedActionDoesNotVerify() throws {
    let app = PicoNodApprovalApp.fresh()
    let request = ApprovalRequest(
        txId: 11,
        generation: 1,
        workspaceId: 3,
        objectId: 44,
        bodyHash: "body",
        summaryHash: "summary",
        nonce: 99
    )
    let displayed = app.display(request)
    var evidence = try app.decide(displayed, action: .nod)
    evidence.action = .reject

    #expect(try !PicoNodApprovalApp.verify(evidence))
}

@Test
func malformedEvidenceFailsClosed() throws {
    let request = ApprovalRequest(
        txId: 1,
        generation: 1,
        workspaceId: 1,
        objectId: 1,
        bodyHash: "body",
        summaryHash: "summary",
        nonce: 1
    )
    let evidence = ApprovalEvidence(
        request: request,
        action: .fence,
        displayedHash: PicoNodApprovalApp.displayedHash(request),
        publicKey: "not-hex",
        signature: "also-not-hex"
    )

    do {
        _ = try PicoNodApprovalApp.verify(evidence)
        Issue.record("malformed evidence unexpectedly verified")
    } catch PicoNodAppError.malformedHex {
    }
}
