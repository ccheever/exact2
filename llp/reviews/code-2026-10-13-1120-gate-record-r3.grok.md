# Review: LLP 1120 §5's as-measured Apple gate round 3, 2026-10-13 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `2c939ec4e847629dd51641a4e034585ea5d9173ef94321589344fff53980bfe7`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** NOT READY.
- **Disposition:** The MAJOR (the 16.5 s launch is a second failed run) taken in the landing commit, after the three-round limit, with the lead's go; its median without that launch corrected to 1513 (the review's 1584 is wrong; IQR 186 and worst 3050 are right). No fourth round.

---

**MAJOR** — Window 2's 16.5 s C launch is counted as a completed run, but five of its feed reads never connected. `llp/1120-network-concurrency-without-threads.rfc.md:543`

The table's window-2 C cell is `1521 (678, 16505), 1 failed`, and gate 5 (`:552`) names one failed launch. The null `zero_to_post` is that one failure. The 16505 ms launch is the other one in `lte-2/C-2`: it did paint, and it logged 27 exchanges. Six of those have no local address, remote address, or protocol: `getServices` and the first five author feeds (`bsky.app`, `jay.bsky.team`, `pfrazee.com`, `emilyliu.me`, `atproto.com`). That is the same shape as the cancelled launch's lines. The other thirteen author feeds and Discover share one `h2` connection, and none of those five URLs was retried. Signed-out Following is those eighteen authors plus Discover (`bsky-measure/data/feeds.ts`), and a failed author feed is marked done and not retried. Gate 5 requires every run to complete all 19 feed reads. This run connected 14. It still supplies the reported worst and the IQR of 678. Dropped, the remaining ten launches have median 1584, IQR 186, and worst 3050.

The round-2 fixes check out against the raw launches. Medians, IQRs (linear interpolation, rounded as the helper does), and worsts match the table, including 178 / 116 / 121 and window 3's A7 blocks 1770, 1613, 1634. Gate 1 fails: window 3's C7 is 10 ms under A7, inside an IQR of 284, and the worst is better. Gates 3 and 4 fail with gate 5, and the window-1 and window-3 pairs cited as what would pass do pass that gate text. Connection counts are one `h2` per launch for C7 and C, seven for A7, and six for A6, counted only where an endpoint was recorded; both partial window-2 C launches are disclosed, and the log time is completion, so overlap is not shown. The window-1 C launch of 27 exchanges is one pagination read short and still has all 19 feed reads. Worker connections carry two to seven of 28 exchanges, and no worker launch lost an exchange. "Not slower" is only the clean gated pairs. The stall's cause is marked a judgement, and the remedies are marked as not shown. The switch stays off because gates 1 and 5 fail.

NOT READY.
