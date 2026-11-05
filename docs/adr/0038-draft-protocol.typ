#import "../template.typ": adr, arch, ext, validation

== ADR-0038 — Draft protocol <adr-0038>

*Status:* proposed. \
*See also:* #arch("mixed-assessments")[architecture], sections #arch("mixed-assessments")[Mixed assessments] and #arch("safe-exam-browser")[Safe Exam Browser]; #adr("0008"), #adr("0015"), #adr("0020"), #adr("0034"), #adr("0037").

=== Context

Every item is autosaved as a draft (#adr("0015")), and a student who restarts #ext("seb")[Safe Exam Browser] gets their drafts back (#arch("safe-exam-browser")[architecture]). What a save means is not written down, and an exam is where it matters most:

- A frozen SEB, a replayed request or a second tab could send an older draft after a newer one. Saved blindly, the older one would replace the newer work without anyone noticing.
- The network can drop in the middle of an exam. What was typed meanwhile must survive a restart of SEB, on a workstation the next student will use.
- The last seconds before the end hold the most typing, and the end comes from the server's clock.

=== Decision

*The server's draft is the truth, saved by revision.* Each write names the revision it was based on. When that is still the current one, the write applies and becomes the next revision. Otherwise the API answers `409 Conflict` with the current draft, and the interface shows both versions so the student chooses. No write is lost without the student seeing it. A write that the server already applied, sent again because its acknowledgement was lost, is acknowledged again. A draft never written is revision 0, with an empty source.

*Drafts go through the gate.* Reading or writing a draft goes through the single gate of #adr("0020"): only while the exercise is open.

*The interface sends a draft 2 s after the last keystroke*, and at least every 10 s while typing continues. That keeps the write budget near 0.4 writes per second per student (#arch("target-load")[target load]).

*Unacknowledged drafts are kept, encrypted, in the browser.* Only the last full state per item is kept, so the buffer stays small. It goes to IndexedDB, encrypted with AES-GCM through WebCrypto, under a key per student and activity that the API hands out after sign-in and that the page holds in memory only. The next student on a shared workstation therefore cannot read it. The buffer is emptied on acknowledgement and on sign-out. While it holds anything, a banner says "Not saved — do not quit SEB".

*The end of a session is announced.* The server sends `closing` on the event stream (#adr("0008")), and the interface sends its drafts at once. Writes received up to a few seconds after the close (a setting, 5 s by default) are still accepted and logged as `draft.grace`. The redirect to SEB's Quit URL waits for the acknowledgement, or for a bounded delay.

=== Consequences

- The drafts table holds one row per student and item, rewritten in place: it does not grow with the number of saves.
- A conflict asks the student to choose, which an exam interface must explain in a few words.
- The grace window lets a write arrive after the close; it is logged so that a dispute can be settled.

#validation(id: "V-0038")[
  Two tabs that write on the same revision: the second one sees `409` and both versions. Cut the network during typing, restart SEB, sign in again: the buffered draft is sent and acknowledged. Sign out and sign in as another student on the same workstation: the previous buffer is gone and unreadable. At the end of a timed session, the last draft typed before the close is saved.
]
