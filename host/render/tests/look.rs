//! The render host under the neutral look (LLP 1115.000 D4 part B). A binary
//! of its own: the look is the process's (the role table's column), and the
//! other tests of this host render Apple's pairs beside it.
use exact_kernel::style::roles::{self, Column};

/// A document is rendered over the shell of the build it belongs to, and a
/// shell built in the neutral look says so in its stylesheet's first rule
/// (`--exact-look: neutral`, host/web/neutral.css; a build's minifier drops
/// the space). The renderer then resolves a role in the neutral column, as
/// the page that takes the document over will. Any other shell leaves the
/// column where every process starts: Apple's.
#[test]
fn a_shell_in_the_neutral_look_renders_the_neutral_column() {
    exact_render::shell_look(
        "<!doctype html><style>html{color-scheme:light dark}</style><div id=\"exact-root\"></div>",
    );
    assert_eq!(roles::column(), Column::Apple);
    exact_render::shell_look("<style>html{color-scheme:light dark}:root{--exact-look:neutral;--exact-label:light-dark(#000000ff,#ffffffff)}</style>");
    assert_eq!(roles::column(), Column::Neutral);
    roles::set_column(Column::Apple);
    exact_render::shell_look("<style>\n  :root { --exact-look: neutral; }\n</style>");
    assert_eq!(roles::column(), Column::Neutral, "as the source writes it");
}
