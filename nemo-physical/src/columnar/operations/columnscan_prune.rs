use crate::columnar::traits::columnscan::ColumnScanCell;
use crate::datatypes::ColumnDataType;
use crate::tabular::operations::triescan_prune::{SharedTrieScanPruneState, TrieScanPruneState};
use std::fmt::Debug;
use std::ops::Range;

use super::super::traits::columnscan::ColumnScan;

/// Forwards calls to this column though it's [`SharedTrieScanPruneState`] to the input trie.
/// See `TrieScanPrune` for more information.
#[derive(Debug)]
pub struct ColumnScanPrune<'a, T>
where
    T: 'a + ColumnDataType,
{
    state: SharedTrieScanPruneState<'a>,
    /// Index this column scan corresponds to in the input trie
    column_scan_index: usize,
    reference_scan: &'a ColumnScanCell<'a, T>,
}

impl<'a, T> ColumnScanPrune<'a, T>
where
    T: 'a + ColumnDataType,
{
    /// Constructs a new column scan. This should not be called directly, but only by `TrieScanPrune`
    pub fn new(
        state: SharedTrieScanPruneState<'a>,
        column_scan_index: usize,
        reference_scan: &'a ColumnScanCell<'a, T>,
    ) -> Self {
        Self {
            state,
            column_scan_index,
            reference_scan,
        }
    }

    unsafe fn exclusively_get_shared_state<'b>(
        state: &SharedTrieScanPruneState<'a>,
    ) -> &'b mut TrieScanPruneState<'a> {
        unsafe { &mut *state.get() }
    }
}

impl<'a, T> Iterator for ColumnScanPrune<'a, T>
where
    T: 'a + ColumnDataType,
{
    type Item = T;

    /// Gets the next output value for a column.
    ///
    /// This is forwarded to `advance()` of [`crate::tabular::operations::triescan_prune::TrieScanPruneState`]`.
    fn next(&mut self) -> Option<Self::Item> {
        let current_layer = self.column_scan_index;

        unsafe {
            let state = Self::exclusively_get_shared_state(&self.state);
            state.advance_on_layer(current_layer, false);
        };

        self.reference_scan.current()
    }
}

impl<'a, T> ColumnScan for ColumnScanPrune<'a, T>
where
    T: 'a + ColumnDataType,
{
    fn seek(&mut self, minimum_value: T) -> Option<T> {
        let current_layer = self.column_scan_index;

        unsafe {
            let state = Self::exclusively_get_shared_state(&self.state);

            // Advance the underlying trie scan, taking layer peeks into account
            // This calls `seek()` and `next()` under the hood, possibly multiple times.
            state.advance_on_layer_with_seek(
                current_layer,
                false,
                self.reference_scan,
                minimum_value,
            )
        }
    }

    fn current(&self) -> Option<T> {
        if unsafe { (*self.state.get()).is_column_peeked(self.column_scan_index) } {
            None
        } else {
            self.reference_scan.current()
        }
    }

    fn reset(&mut self) {
        unimplemented!("The reset function is currently not implemented for ColumnScanPrune. If required, this could probably be implemented in the future. Doing so would require not violating the guarantees of the TrieScanPrune.");
    }

    fn pos(&self) -> Option<usize> {
        unimplemented!("This function is not implemented for column operators");
    }
    fn narrow(&mut self, _interval: Range<usize>) {
        unimplemented!("This function is not implemented for column operators");
    }
}
