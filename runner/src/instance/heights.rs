//! Compact prefix sums for measured rows. Updates and offset lookup are O(log N).
#[derive(Debug, Default)]
pub(super) struct Heights {
    values: Vec<f64>,
    sums: Vec<f64>,
}

impl Heights {
    pub fn new(values: Vec<f64>) -> Self {
        let mut sums = vec![0.0; values.len() + 1];
        for (index, height) in values.iter().enumerate() {
            let i = index + 1;
            sums[i] += height;
            let parent = i + (i & i.wrapping_neg());
            if parent < sums.len() {
                sums[parent] += sums[i];
            }
        }
        Self { values, sums }
    }

    pub fn value(&self, index: usize) -> f64 {
        self.values[index]
    }

    pub fn set(&mut self, index: usize, height: f64) -> bool {
        let delta = height - self.values[index];
        if delta == 0.0 {
            return false;
        }
        self.values[index] = height;
        let mut i = index + 1;
        while i < self.sums.len() {
            self.sums[i] += delta;
            i += i & i.wrapping_neg();
        }
        true
    }

    /// Offset of a row, or total extent for index == len.
    pub fn offset(&self, mut index: usize) -> f64 {
        let mut sum = 0.0;
        while index != 0 {
            sum += self.sums[index];
            index &= index - 1;
        }
        sum
    }

    /// First row whose bottom is strictly after the offset; len past the end.
    pub fn locate(&self, offset: f64) -> usize {
        let mut index = 0;
        let mut sum = 0.0;
        let mut step = self.sums.len().next_power_of_two();
        while step != 0 {
            let next = index + step;
            if next < self.sums.len() && sum + self.sums[next] <= offset {
                sum += self.sums[next];
                index = next;
            }
            step >>= 1;
        }
        index
    }
}
