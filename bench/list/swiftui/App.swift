// SwiftUI version of Expo PR 49975's DataListForEachScreen (listbench).
// The most ordinary SwiftUI: a List with Sections and ForEach over the data,
// .onDelete / .onMove, selection by tag, the edit mode from a button.
import SwiftUI

struct Message: Identifiable, Hashable {
    let id: String
    let index: Int
    let text: String
}

private let texts = [
    "Are we still meeting for coffee?",
    "Yes! I found a place near the park. We can walk over afterward if the weather holds.",
    "Things to bring:\nCamera\nA warm jacket\nSomething for the picnic",
    "Sounds good. See you there ☕️",
]
private let allMessages = (0..<10_000).map { Message(id: "message-\($0)", index: $0, text: texts[$0 % 4]) }

@main
struct ListBenchApp: App {
    var body: some Scene { WindowGroup { ContentView() } }
}

struct ContentView: View {
    @State private var saved: Set<String> = []
    @State private var largeBuffer = false
    @State private var reversed = false
    @State private var items = Array(allMessages[0..<5000])
    @State private var archive = Array(allMessages[5000...])
    @State private var selection: Set<String> = []
    @State private var editMode: EditMode = .inactive

    var body: some View {
        VStack(spacing: 0) {
            VStack(spacing: 12) {
                HStack(spacing: 12) {
                    Button {
                        reversed.toggle()
                        items.reverse()
                        archive.reverse()
                    } label: {
                        Label(reversed ? "Restore order" : "Reverse order", systemImage: "arrow.up.arrow.down")
                    }
                    .buttonStyle(.bordered)
                    Spacer()
                    Button {
                        editMode = editMode.isEditing ? .inactive : .active
                    } label: {
                        Label(editMode.isEditing ? "Done" : "Edit",
                              systemImage: editMode.isEditing ? "checkmark" : "slider.horizontal.3")
                    }
                    .buttonStyle(editMode.isEditing ? AnyPrimitiveButtonStyle(.borderedProminent) : AnyPrimitiveButtonStyle(.bordered))
                }
                Toggle(isOn: $largeBuffer) {
                    Text("Larger scroll buffer")
                    Text("\(largeBuffer ? 20 : 10) extra rows on each side")
                }
            }
            .padding(.horizontal, 20)
            .padding(.vertical, 12)

            List(selection: $selection) {
                Section {
                    VStack(alignment: .leading, spacing: 12) {
                        Text("\((items.count + archive.count).formatted()) messages")
                            .font(.system(.title2, design: .rounded, weight: .bold))
                            .monospacedDigit()
                        HStack(spacing: 16) {
                            Label("\(saved.count) saved", systemImage: "bookmark")
                            Label("\(selection.count) selected", systemImage: "checkmark.circle")
                        }
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                    }
                    .padding(.vertical, 8)
                } footer: {
                    Text("Save a message, scroll away, and come back. Your bookmarks stay.")
                }

                Section {
                    ForEach(items) { item in row(item) }
                        .onDelete { items.remove(atOffsets: $0) }
                        .onMove { items.move(fromOffsets: $0, toOffset: $1) }
                } header: {
                    Text("Inbox · \(items.count.formatted())")
                } footer: {
                    Text("Swipe to delete. Use Edit to select messages or drag to reorder.")
                }

                Text("Inbox and Archive each keep their own scroll buffer.")
                    .font(.footnote)
                    .foregroundStyle(.secondary)

                Section {
                    ForEach(archive) { item in row(item) }
                        .onDelete { archive.remove(atOffsets: $0) }
                        .onMove { archive.move(fromOffsets: $0, toOffset: $1) }
                } header: {
                    Text("Archive · \(archive.count.formatted())")
                }

                Section {
                    Label("You’re all caught up", systemImage: "checkmark.circle")
                        .foregroundStyle(.secondary)
                } footer: {
                    Text("Fast scrolling may briefly show placeholders while the next rows load.")
                }
            }
            .listStyle(.insetGrouped)
            .environment(\.editMode, $editMode)
        }
        .background(Color(.systemGroupedBackground))
    }

    private func row(_ item: Message) -> some View {
        let isSaved = saved.contains(item.id)
        return VStack(alignment: .leading, spacing: 10) {
            HStack(spacing: 8) {
                Label("Message \(item.index + 1)",
                      systemImage: item.index % 2 == 1 ? "person.crop.circle" : "bubble.left.and.bubble.right")
                    .font(.headline)
                Spacer(minLength: 0)
                Button {
                    if isSaved { saved.remove(item.id) } else { saved.insert(item.id) }
                } label: {
                    Label(isSaved ? "Unsave message" : "Save message",
                          systemImage: isSaved ? "bookmark.fill" : "bookmark")
                }
                .buttonStyle(.borderless)
                .labelStyle(.iconOnly)
                .tint(isSaved ? Color(.systemOrange) : Color(.secondaryLabel))
                .frame(minWidth: 44, minHeight: 44)
                .accessibilityLabel("\(isSaved ? "Unsave" : "Save") message \(item.index + 1)")
            }
            Text(item.text)
                .font(.body)
            if isSaved {
                Label("Saved for later", systemImage: "bookmark.fill")
                    .font(.caption)
                    .foregroundStyle(Color(.systemOrange))
            }
            if item.index % 5 == 0 {
                Label("Weekend itinerary.pdf · 248 KB", systemImage: "doc.text")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .padding(.horizontal, 10)
                    .padding(.vertical, 8)
                    .background(Color(.tertiarySystemFill), in: RoundedRectangle(cornerRadius: 8))
            }
        }
        .padding(.vertical, 6)
        .tag(item.id)
    }
}

/// Lets one `.buttonStyle` switch between bordered and borderedProminent.
struct AnyPrimitiveButtonStyle: PrimitiveButtonStyle {
    private let make: (Configuration) -> AnyView
    init<S: PrimitiveButtonStyle>(_ style: S) { make = { AnyView(style.makeBody(configuration: $0)) } }
    func makeBody(configuration: Configuration) -> some View { make(configuration) }
}
