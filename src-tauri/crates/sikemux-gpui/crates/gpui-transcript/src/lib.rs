//! A streaming chat view for GPUI.
//!
//! - [`Rows`] holds a chat's rows by stable id: put, update, drop from, keep, prepend a page.
//! - [`Markdown`] holds one message's blocks and applies the patches its text
//!   grows by, so only the blocks a patch reaches are laid out again.
//! - [`markdown`] draws those blocks.
//! - [`Transcript`] draws rows in a windowed list anchored to the bottom,
//!   applies the changes pushed to it once per frame, and asks for earlier
//!   pages as the reader nears the top.

mod draw;
mod markdown;
mod rows;
mod transcript;

pub use draw::{markdown, plain, DrawFence, DrawImage, Fence, OnLink, Style};
pub use markdown::{Align, Kind, Markdown, Marks, Node, Piece, Rich, Table};
pub use rows::{Edit, Row, Rows};
pub use transcript::{Change, Transcript};
