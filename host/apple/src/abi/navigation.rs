use super::*;

impl<D: DataSource> Bridge<D> {
    /// Read the first root's literal viewport policy without settling a runner.
    /// A zero length selects the embedded plan; otherwise read the input buffer.
    pub fn plan_covers_viewport(&self, len: usize, embedded: &'static [u8]) -> bool {
        let bytes = if len == 0 {
            embedded
        } else if let Some(bytes) = self.input.get(..len) {
            bytes
        } else {
            return false;
        };
        let Ok(plan) = exact_plan::Plan::decode(bytes) else {
            return false;
        };
        let Some(root) = plan
            .nodes
            .iter()
            .filter(|n| n.parent.is_none() && n.arm.is_none())
            .min_by_key(|n| n.order)
        else {
            return false;
        };
        // A preceding conditional root cannot be selected without app state.
        if plan
            .regions
            .iter()
            .any(|r| r.parent.is_none() && r.arm.is_none() && r.order < root.order)
        {
            return false;
        }
        plan.bindings
            .iter()
            .skip(root.bindings.start as usize)
            .take(root.bindings.len as usize)
            .find(|b| {
                b.kind == exact_plan::BindingKind::Prop
                    && b.id == exact_kernel::PropId::ViewportFit as u16
            })
            .is_some_and(|b| {
                use exact_plan::Opcode;
                match plan.code(b.expr) {
                    [op, a, b, c, d, ret]
                        if *op == Opcode::Str as u8 && *ret == Opcode::Return as u8 =>
                    {
                        plan.str(exact_plan::StrId(u32::from_le_bytes([*a, *b, *c, *d]))) == "cover"
                    }
                    _ => false,
                }
            })
    }

    /// `exact_host_back`: the platform's own Back from visit `id` (LLP 1115
    /// D5), a batch.
    pub fn host_back(&mut self, id: u64, now_ms: f64) -> u32 {
        let out = match self.host.as_mut() {
            Some(h) => h.host_back(id, now_ms),
            None => not_booted(),
        };
        self.emit(out)
    }

    /// Complete a Back-history jump between visit identities.
    pub fn host_back_to(&mut self, source: u64, destination: u64, now_ms: f64) -> u32 {
        let out = self
            .host
            .as_mut()
            .map_or_else(not_booted, |h| h.host_back_to(source, destination, now_ms));
        self.emit(out)
    }

    /// Follow the UTF-8 location in the owned input buffer.
    pub fn host_go(&mut self, len: usize, now_ms: f64) -> u32 {
        let location = String::from_utf8_lossy(&self.input[..len.min(self.input.len())]);
        let out = self
            .host
            .as_mut()
            .map_or_else(not_booted, |h| h.host_go(&location, now_ms));
        self.emit(out)
    }
}
