//! Original prose. Every string here is measured by `font.rs` before a host
//! sees it, so its characters must stay inside `build.rs`'s charset.

/// The balls scene: one paragraph the spheres pass through.
pub const BALLS: &str = "The printer’s apprentice learned the trade by weight before she learned it by eye. A line of type had a heft in the stick, and the stick told you when the measure was full long before you counted the ems. Later, when the shop bought its first machine, the old compositor kept a case of foundry type in the back room and set the shop’s own stationery by hand, because a machine could cast a line but it could not feel one. She thought of that often in the years after, whenever some new system promised to lay out a page without ever measuring anything, and she found herself reaching for a ruler. Every good page, she decided, is a page that was measured first and drawn second; the ones that look wrong are almost always the ones where somebody guessed.";

/// The dragon scene: a bestiary entry, its initial letter supplied by the drop cap.
pub const DRAGON: &str = "f the dragon the old books say much and agree on little. It is written that the creature sleeps for a century at a time beneath a hill of its own making, and that the hill is warm to the touch in winter, so that shepherds learn to bring their flocks there when the snow is deep. It is written elsewhere that it never sleeps at all, and that the warmth of the hill is only the memory of an older fire. Both accounts are given by men who claim to have seen it, and both are copied here without correction, for the copyist is not the judge.\n\nOf its shape there is more agreement. The neck is long and carried low, the wings are folded like a cloak when it walks, and the tail is longer than the body and ends in a blade of horn. The scales along the back rise into a comb, which the animal raises when it is angry and lays flat when it is content, and by this sign alone a traveller may know whether to go on or go back. Its colour is the colour of the country it lives in, which is why every province describes a different beast and none of them is wrong.\n\nIt is fond of gold, but not, as the ballads say, for the gold itself. A dragon cannot see its own hoard, being always upon it; what it loves is the sound the coins make when a thief disturbs them, for that sound is the only company it keeps. Feed it nothing. Speak to it plainly. Do not, whatever the ballads say, attempt to move it, for it will not go where it is pushed, and the words on this page will only close around it again.\n\nThe scribe adds, in a later hand, that he has drawn the animal from the description alone and begs the reader’s pardon if it is not exactly like.";

/// The magazine and spread scenes: a long article with paragraph breaks.
pub const ARTICLE: &str = "There is a moment in every layout when the page stops being a picture and becomes a measurement. Until then it is a sketch: a headline here, a column there, a photograph roughly this big. The sketch is honest about what it does not know. It does not know how long the text is, so it draws a grey block and hopes. It does not know where the lines will break, so it draws straight rules and calls them columns. The moment the real words arrive, every one of those guesses is tested at once, and most of them fail in small ways that add up to a page that looks almost right.\n\nThe usual answer is to let the page find its own shape. Pour the text in, let it flow, and see what happens. That works when there is one column and nothing in the way. It stops working the moment the design asks a question the flow cannot answer on its own. How tall will this card be? Which column will the quotation land in? If the photograph sits here, does the article still end on this page? Each of those questions needs a number before the page can be drawn, and the number is the height of some text at some width.\n\nFor most of the history of printing that number was found by setting the type. A compositor filled a measure, counted the lines, and knew. The knowledge was expensive, because it cost the whole job to get it, but it was exact. The page was measured first and drawn second. Software reversed the order without meaning to: draw the page, then ask the machine what it did, then draw it again. Anyone who has watched a web page settle into place, jumping twice as its pictures and paragraphs arrive, has watched that reversal happen in public.\n\nThe idea behind a measured layout is embarrassingly simple. Measure every word once, in the face and size it will be set in, and keep the numbers. After that, deciding how tall a paragraph will be at any width is addition. You add advances until the line is full, start a new line, and count. There is no page to draw and throw away, no waiting for a machine to report back, no second pass. The cost is paid at the beginning, when the words are known and the layout is not, which is exactly when a design can still change its mind.\n\nOnce heights are cheap, a whole class of layouts becomes ordinary. A wall of cards can be balanced across columns before a single card is drawn, because every card’s height is known in advance. A column can be cut at a line boundary rather than a character count. A pull quote can be placed where the eye wants it, and the article can be told to make room, because the room it needs is a number too. None of these are new ideas. They are the ideas of every good magazine of the last century, made cheap enough to do again on every resize.\n\nThe interesting part is not the arithmetic. Adding widths is a few dozen lines of code in any language. The interesting part is agreement. The numbers are only useful if the thing that finally draws the text breaks the lines in the same places the arithmetic did. A measurement that is off by a hair on one line will, sooner or later, push a word onto a new line that the arithmetic did not expect, and then the card is one line taller than the wall thought it was. Agreement is what the whole scheme rests on, and agreement is a matter of using the same font, the same rules, and the same rounding at both ends.\n\nOn this page both ends are the same. The words were measured with the advances read out of the font file the page is set in, and the lines were broken with the same walker every host of this application uses to flow text around a shape. When the text is drawn, it is drawn from those breaks. There is no second engine to disagree, which is another way of saying there is nothing to wait for.\n\nDrag the edge of this page and watch it re-cut its columns. Nothing is being re-measured. The widths were known before you touched it; only the sums change. That is the entire trick, and it is enough.";

/// Card titles for the masonry and wall scenes.
pub const TITLES: [&str; 24] = [
    "On measuring first",
    "The stick and the machine",
    "A short history of the em",
    "Why columns are cut at lines",
    "Balanced by arithmetic",
    "What the compositor knew",
    "Cards of uneven height",
    "The page that jumped twice",
    "Agreement between two ends",
    "Room for a quotation",
    "A note on rounding",
    "Fonts as tables of numbers",
    "Guessing versus knowing",
    "Twelve hundred cards",
    "The wall that did not wait",
    "Drawn from the same breaks",
    "Slack, and how much",
    "A ruler in the drawer",
    "Every resize is free",
    "The shape of a column",
    "One walker, three hosts",
    "Where the lines fall",
    "Heights before pixels",
    "The sketch and the measure",
];

/// Sentences the card generator draws from.
pub const SENTENCES: [&str; 40] = [
    "Every card on this wall was measured before it was placed.",
    "The height you see is the height the arithmetic predicted.",
    "Nothing here waited for a layout pass to report back.",
    "The compositor filled the measure and counted the lines.",
    "A guess is cheap until it is wrong on every page.",
    "Widths are read from the font file once and kept.",
    "Adding advances is faster than asking a renderer.",
    "The columns are balanced by placing each card where the stack is shortest.",
    "Resize the window and the wall re-balances without re-measuring anything.",
    "The same walker breaks these lines on the web, on macOS, and on Linux.",
    "Kerning is included, because the hosts include it.",
    "An em is the width of the type size, and everything else is a fraction of it.",
    "When two engines disagree, one of them draws a blank last line.",
    "Agreement is a matter of the same font, the same rules, and the same rounding.",
    "The old shop kept a case of foundry type for the jobs that mattered.",
    "A machine could cast a line but it could not feel one.",
    "Slack is what you add when you do not trust the number.",
    "This card is exactly as tall as it needs to be.",
    "There is no grey block here standing in for text that has not arrived.",
    "Pour the text in and see what happens, said nobody who had a deadline.",
    "The wall knows how tall it is before the first card is drawn.",
    "A pull quote needs a number, and the number is the height of some text.",
    "Only the cards on screen exist as views; the rest are positions.",
    "Scroll as fast as you like; the positions were never in doubt.",
    "A page that was measured first rarely needs to be drawn twice.",
    "Ligatures and kerning are the two places a sum can drift from a shape.",
    "The advances are in font units; the size turns them into pixels.",
    "A title in the bold face is measured with the bold face’s table.",
    "Every line boundary here is a line boundary the host will agree with.",
    "The footer of each card reports what the arithmetic expected.",
    "Twelve hundred paragraphs cost a few milliseconds to measure.",
    "Position is a fold over heights; nothing else is needed.",
    "A resize changes the sums and nothing else.",
    "The interesting part is not the arithmetic but the agreement.",
    "Draw it from the breaks you computed and there is nothing to wait for.",
    "Text around a shape is the same walker with a narrower line.",
    "The dragon will not go where it is pushed.",
    "Measure once, at the beginning, when the layout can still change its mind.",
    "The ruler stayed in the drawer, but she reached for it every day.",
    "It is enough.",
];
