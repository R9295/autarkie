use std::ops::Range;

use autarkie::Visitor;

pub fn calculate_subslice_bounds(len: usize, max: usize, visitor: &mut Visitor) -> Range<usize> {
    // minus 1 because we zero index and len is always +1
    let start = visitor.random_range(0, len - 1);
    let mut end = visitor.random_range(start, len);
    if end - start > max {
        end = start + max;
    }
    start..end
}
