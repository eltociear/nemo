use super::super::traits::columnscan::{ColumnScan, ColumnScanCell};
use crate::datatypes::ColumnDataType;
use std::{fmt::Debug, ops::Range};

/// [`ColumnScan`] which allows its sub scan to only jump to the value pointed to by a reference scan
#[derive(Debug)]
pub struct ColumnScanEqualColumn<'a, T>
where
    T: 'a + ColumnDataType,
{
    /// `value_scan` may only jump to the value currently pointed to by this scan
    reference_scan: &'a ColumnScanCell<'a, T>,

    /// The sub scan
    value_scan: &'a ColumnScanCell<'a, T>,

    /// Current value of this scan
    current_value: Option<T>,
}
impl<'a, T> ColumnScanEqualColumn<'a, T>
where
    T: 'a + ColumnDataType,
{
    /// Constructs a new [`ColumnScanEqualColumn`].
    pub fn new(
        reference_scan: &'a ColumnScanCell<'a, T>,
        value_scan: &'a ColumnScanCell<'a, T>,
    ) -> ColumnScanEqualColumn<'a, T> {
        ColumnScanEqualColumn {
            reference_scan,
            value_scan,
            current_value: None,
        }
    }
}

impl<'a, T> Iterator for ColumnScanEqualColumn<'a, T>
where
    T: 'a + ColumnDataType + Eq,
{
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_value.is_some() {
            // Since we know that the next value must be bigger, we can return None
            self.current_value = None;
            return None;
        }

        let reference_value = self.reference_scan.current()?;
        let next_value_opt = self.value_scan.seek(reference_value);

        if let Some(next_value) = next_value_opt {
            if next_value == reference_value {
                self.current_value = next_value_opt;
            } else {
                self.current_value = None;
            }
        } else {
            self.current_value = None;
        }
        self.current_value
    }
}

impl<'a, T> ColumnScan for ColumnScanEqualColumn<'a, T>
where
    T: 'a + ColumnDataType + Eq,
{
    fn seek(&mut self, value: T) -> Option<T> {
        let reference_value = self.reference_scan.current()?;
        if value > reference_value {
            self.current_value = None;
            None
        } else {
            self.next()
        }
    }

    fn current(&self) -> Option<T> {
        self.current_value
    }

    fn reset(&mut self) {
        self.current_value = None;
    }

    fn pos(&self) -> Option<usize> {
        unimplemented!("This functions is not implemented for column operators");
    }
    fn narrow(&mut self, _interval: Range<usize>) {
        unimplemented!("This functions is not implemented for column operators");
    }
}

#[cfg(test)]
mod test {
    use crate::columnar::{
        column_types::vector::ColumnVector,
        traits::{
            column::Column,
            columnscan::{ColumnScan, ColumnScanCell, ColumnScanEnum},
        },
    };

    use super::ColumnScanEqualColumn;
    use test_log::test;

    #[test]
    fn test_u64() {
        let ref_col = ColumnVector::new(vec![0u64, 4, 7]);
        let val_col = ColumnVector::new(vec![1u64, 4, 8]);

        let ref_iter = ColumnScanCell::new(ColumnScanEnum::ColumnScanVector(ref_col.iter()));
        let val_iter = ColumnScanCell::new(ColumnScanEnum::ColumnScanVector(val_col.iter()));

        ref_iter.seek(4);

        let mut equal_scan = ColumnScanEqualColumn::new(&ref_iter, &val_iter);
        assert_eq!(equal_scan.current(), None);
        assert_eq!(equal_scan.next(), Some(4));
        assert_eq!(equal_scan.current(), Some(4));
        assert_eq!(equal_scan.next(), None);
        assert_eq!(equal_scan.current(), None);

        let ref_iter = ColumnScanCell::new(ColumnScanEnum::ColumnScanVector(ref_col.iter()));
        let val_iter = ColumnScanCell::new(ColumnScanEnum::ColumnScanVector(val_col.iter()));

        ref_iter.seek(7);

        let mut equal_scan = ColumnScanEqualColumn::new(&ref_iter, &val_iter);
        assert_eq!(equal_scan.current(), None);
        assert_eq!(equal_scan.next(), None);
        assert_eq!(equal_scan.current(), None);
    }
}
