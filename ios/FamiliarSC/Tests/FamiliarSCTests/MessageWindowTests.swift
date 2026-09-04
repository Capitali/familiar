import XCTest
@testable import FamiliarSC

/// The message window: advice and proposals from the journal, each proposal with its state
/// from proposals ∪ approvals — open, approved, denied, lapsed.
final class MessageWindowTests: XCTestCase {
    func testProposalStatesFromTheStore() {
        let s = Fixtures.store
        let items = MessageWindow.build(journal: Fixtures.journal().entries, proposals: s.proposals(), approvals: s.approvals(), nowTick: 251)
        XCTAssertEqual(items.count, 3)
        guard case .advice(let would, let why) = items[0].kind else { return XCTFail() }
        XCTAssertEqual(items[0].surface, .marketBuy)
        XCTAssertEqual(would, "buy 30 gravy-base at 20")
        XCTAssertEqual(why, "margin 25% at velvet-array")
        guard case .proposal(let id1, _, _, let exp1, let st1) = items[1].kind else { return XCTFail() }
        XCTAssertEqual(id1, "p-0123456789abcdef"); XCTAssertEqual(exp1, 205); XCTAssertEqual(st1, .lapsed)
        guard case .proposal(let id2, let would2, _, _, let st2) = items[2].kind else { return XCTFail() }
        XCTAssertEqual(id2, "p-fedcba9876543210"); XCTAssertEqual(would2, "sell 40 ore at 22")
        XCTAssertEqual(st2, .approved(at: 1700002950))
        XCTAssertEqual(items.filter(\.needsTheCaptain).count, 0)
    }

    func testAnUnansweredProposalIsOpenUntilItsTickThenLapsed() {
        let j = Fixtures.journal().entries
        let open = MessageWindow.build(journal: j, proposals: Fixtures.store.proposals(), approvals: [], nowTick: 212)
        guard case .proposal(_, _, _, _, let st) = open[2].kind else { return XCTFail() }
        XCTAssertEqual(st, .open)
        XCTAssertTrue(open[2].needsTheCaptain)
        let late = MessageWindow.build(journal: j, proposals: Fixtures.store.proposals(), approvals: [], nowTick: 215)
        guard case .proposal(_, _, _, _, let st2) = late[2].kind else { return XCTFail() }
        XCTAssertEqual(st2, .lapsed)
        let unknown = MessageWindow.build(journal: j, proposals: [], approvals: [], nowTick: nil)
        guard case .proposal(_, _, _, _, let st3) = unknown[2].kind else { return XCTFail() }
        XCTAssertEqual(st3, .open, "no clock, no lapse line: still open")
    }

    func testDenialIsTheLastWord() {
        let a = [Approval(id: "p-fedcba9876543210", approved: true, at: 1), Approval(id: "p-fedcba9876543210", approved: false, at: 2)]
        let items = MessageWindow.build(journal: Fixtures.journal().entries, proposals: Fixtures.store.proposals(), approvals: a, nowTick: 211)
        guard case .proposal(_, _, _, _, let st) = items[2].kind else { return XCTFail() }
        XCTAssertEqual(st, .denied(at: 2))
    }

    func testApprovalLineIsTheRustShape() {
        XCTAssertEqual(MessageWindow.approvalLine(id: "p-1", approved: true, at: 5), #"{"id":"p-1","approved":true,"at":5}"#)
        let back = try! JSONDecoder().decode(Approval.self, from: Data(MessageWindow.approvalLine(id: "p-1", approved: false, at: 6).utf8))
        XCTAssertEqual(back, Approval(id: "p-1", approved: false, at: 6))
    }
}
