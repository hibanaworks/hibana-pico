import PicoNodAppCore
import SwiftUI

@main
struct PicoNodApp: App {
    var body: some Scene {
        WindowGroup {
            ApprovalView(
                model: ApprovalViewModel(
                    request: ApprovalRequest(
                        txId: 1,
                        generation: 1,
                        workspaceId: 1,
                        objectId: 1,
                        bodyHash: "body-hash-from-server",
                        summaryHash: "summary-hash-from-server",
                        nonce: 42
                    )
                )
            )
        }
    }
}

@MainActor
final class ApprovalViewModel: ObservableObject {
    let app: PicoNodApprovalApp
    let request: ApprovalRequest
    let displayed: DisplayedIntent

    @Published var lastEvidence: ApprovalEvidence?
    @Published var errorText: String?

    init(request: ApprovalRequest, app: PicoNodApprovalApp = .fresh()) {
        self.app = app
        self.request = request
        self.displayed = app.display(request)
    }

    func decide(_ action: ApprovalAction) {
        do {
            lastEvidence = try app.decide(displayed, action: action)
            errorText = nil
        } catch {
            lastEvidence = nil
            errorText = String(describing: error)
        }
    }
}

struct ApprovalView: View {
    @StateObject var model: ApprovalViewModel

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text("Pico Nod")
                .font(.title)
                .fontWeight(.semibold)

            VStack(alignment: .leading, spacing: 8) {
                label("Workspace", "\(model.request.workspaceId)")
                label("Transaction", "\(model.request.txId)")
                label("Body", model.request.bodyHash)
                label("Summary", model.request.summaryHash)
                label("Displayed", model.displayed.displayedHash)
            }
            .font(.system(.body, design: .monospaced))

            HStack(spacing: 12) {
                Button("Nod") { model.decide(.nod) }
                    .buttonStyle(.borderedProminent)
                Button("Reject") { model.decide(.reject) }
                    .buttonStyle(.bordered)
                Button("Fence") { model.decide(.fence) }
                    .buttonStyle(.bordered)
            }

            if let lastEvidence = model.lastEvidence {
                Text(lastEvidence.action.rawValue.uppercased())
                    .font(.headline)
                Text(lastEvidence.signature)
                    .font(.system(.caption, design: .monospaced))
                    .textSelection(.enabled)
            }

            if let errorText = model.errorText {
                Text(errorText)
                    .foregroundStyle(.red)
            }
        }
        .padding(24)
        .frame(minWidth: 360)
    }

    private func label(_ title: String, _ value: String) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(title).font(.caption).foregroundStyle(.secondary)
            Text(value).lineLimit(3)
        }
    }
}
