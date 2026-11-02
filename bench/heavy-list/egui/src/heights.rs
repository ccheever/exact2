// Prefix sums let show_viewport find a row without laying out preceding messages.
pub struct Heights {
    values: Vec<f32>,
    sums: Vec<f64>,
}

impl Heights {
    pub fn new(count: usize) -> Self {
        let mut heights = Self {
            values: vec![0.; count],
            sums: vec![0.; count + 1],
        };
        for index in 0..count {
            heights.set(index, 330.);
        }
        heights
    }

    pub fn set(&mut self, index: usize, height: f32) {
        let delta = f64::from(height - self.values[index]);
        self.values[index] = height;
        let mut node = index + 1;
        while node < self.sums.len() {
            self.sums[node] += delta;
            node += node & node.wrapping_neg();
        }
    }

    pub fn top(&self, index: usize) -> f32 {
        let (mut node, mut sum) = (index, 0.);
        while node > 0 {
            sum += self.sums[node];
            node &= node - 1;
        }
        sum as f32
    }

    pub fn total(&self) -> f32 {
        self.top(self.values.len())
    }

    pub fn row_at(&self, offset: f32) -> usize {
        let mut low = 0;
        let mut high = self.values.len();
        while low + 1 < high {
            let middle = (low + high) / 2;
            if self.top(middle) <= offset {
                low = middle;
            } else {
                high = middle;
            }
        }
        low
    }
}
