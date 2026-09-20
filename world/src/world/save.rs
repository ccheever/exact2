use super::*;
use crate::data::{limits, MAX_LOAD_ENTITIES};

#[derive(Default)]
pub(super) struct Free(pub BTreeSet<u32>);
impl Data for Free {
    fn write(&self, w: &mut dyn Writer) {
        w.begin_seq(self.0.len());
        for i in &self.0 {
            w.claim_decoded(64);
            if w.stopped() {
                break;
            }
            w.item();
            i.write(w);
        }
        w.end_seq();
    }
    fn read(&mut self, r: &mut dyn Reader) -> Result<(), DataError> {
        r.begin_seq()?;
        if r.sequence_len().is_some_and(|n| n > MAX_LOAD_ENTITIES) {
            return Err(DataError::new("free count exceeds entity limit"));
        }
        self.0.clear();
        let mut last = None;
        while r.item()? {
            if self.0.len() == MAX_LOAD_ENTITIES {
                return Err(DataError::new("free count exceeds entity limit"));
            }
            let mut i = 0u32;
            i.read(r)?;
            if last.is_some_and(|old| old >= i) {
                return Err(DataError::new("free list is not strictly ordered"));
            }
            r.claim(64)?;
            self.0.insert(i);
            last = Some(i);
        }
        Ok(())
    }
}
impl Data for State {
    fn write(&self, w: &mut dyn Writer) {
        w.begin_struct();
        w.field("tick");
        self.tick.write(w);
        w.field("hz");
        self.hz.write(w);
        w.field("seed");
        self.seed.write(w);
        w.field("slots");
        self.slots.write(w);
        w.field("free");
        self.free.write(w);
        w.field("busy");
        self.busy.borrow().write(w);
        if !self.work.borrow().is_empty() {
            w.field("work");
            self.work.borrow().write(w);
        }
        w.end_struct();
    }
    fn read(&mut self, r: &mut dyn Reader) -> Result<(), DataError> {
        r.begin_struct()?;
        while let Some(field) = r.field()? {
            match field {
                "tick" => self.tick.read(r),
                "hz" => self.hz.read(r),
                "seed" => self.seed.read(r),
                "slots" => limits::read_vec(r, &mut self.slots, MAX_LOAD_ENTITIES),
                "free" => self.free.read(r),
                "busy" => limits::read_vec(r, self.busy.get_mut(), 64),
                "work" => limits::read_map(r, self.work.get_mut(), 64, 256),
                _ => r.skip(),
            }
            .map_err(|e| e.at(field))?;
        }
        Ok(())
    }
}

impl World {
    pub(crate) fn read_publications(&mut self, r: &mut bin::Decoder<'_>) -> Result<(), DataError> {
        r.begin_struct()?;
        let mut budget = crate::json::LIMIT;
        let values = self.published.get_mut();
        while let Some(key) = r.field()? {
            if values.len() == 256 || key.len() > 256 {
                return Err(DataError::new("publication count/key limit"));
            }
            r.claim(64 + key.len())?;
            let value = crate::Published::read_bounded(r, &mut budget, 0)?;
            values.insert(key.into(), value);
        }
        self.published_cost.set(crate::json::LIMIT - budget);
        Ok(())
    }
}

#[cfg(test)]
mod publication_tests {
    use super::*;
    use crate::counting;
    #[test]
    fn nested_publications_charge_before_allocating_declared_children_or_text() {
        for value in [
            crate::Published::List(vec![crate::Published::Unit; 100_000]),
            crate::Published::Str("x".repeat(1_000_000)),
            crate::Published::Object(BTreeMap::from([(
                "x".repeat(100_000),
                crate::Published::Unit,
            )])),
        ] {
            let mut out = bin::Encoder::default();
            out.begin_struct();
            out.key("x");
            value.write(&mut out);
            out.end_struct();
            let bytes = out.finish().unwrap();
            let mut world = World::new(60, 0);
            let mut r = bin::Decoder::new(&bytes);
            let (result, counts) = counting::measure(|| world.read_publications(&mut r));
            println!("publication refusal: {counts:?}");
            assert!(result.is_err());
            assert!(counts.1 < 20_000, "late allocation: {counts:?}");
        }
        let mut out = bin::Encoder::default();
        out.begin_struct();
        out.key("x");
        crate::Published::List(vec![crate::Published::Unit; 1023]).write(&mut out);
        out.end_struct();
        let bytes = out.finish().unwrap();
        let mut w = World::new(60, 0);
        w.read_publications(&mut bin::Decoder::new(&bytes)).unwrap();
        assert_eq!(w.publications().len(), 1);
    }
    #[test]
    fn nested_reservation_chain_cannot_spend_outstanding_sibling_allowances() {
        use crate::Published::{List, Unit};
        for depth in [1, 8, 40, 80] {
            let mut value = Unit;
            for level in (0..depth).rev() {
                let mut items = vec![Unit; 1023 - level];
                items[0] = value;
                value = List(items);
            }
            let mut out = bin::Encoder::default();
            out.begin_struct();
            out.key("x");
            value.write(&mut out);
            out.end_struct();
            let bytes = out.finish().unwrap();
            let mut world = World::new(60, 0);
            let mut r = bin::Decoder::new(&bytes);
            let (result, counts) = counting::measure(|| world.read_publications(&mut r));
            println!("reservation chain depth={depth}: {counts:?}");
            assert_eq!(result.is_ok(), depth == 1);
            assert!(
                counts.1 < 60_000,
                "outstanding sibling reservations escaped: {counts:?}"
            );
        }
        // Exactly 1,024 values, with a reserved child that has its own child.
        let mut items = vec![Unit; 1022];
        items[0] = List(vec![Unit]);
        let source = World::new(60, 0);
        source.publish("x", List(items)).unwrap();
        let saved = source.save().unwrap();
        let mut loaded = World::new(60, 0);
        loaded.load(&saved).unwrap();
        assert_eq!(loaded.save().unwrap(), saved);
    }
    #[test]
    fn wide_unit_default_component_refuses_before_allocating_a_page() {
        #[allow(clippy::large_enum_variant)]
        #[derive(Default, crate::Component)]
        enum Wide {
            #[default]
            Empty,
            Full([[[u8; 32]; 32]; 32]),
        }
        let mut source = World::new(60, 0);
        source.register::<Wide>().unwrap();
        for _ in 0..129 {
            source.spawn(()).unwrap();
        }
        for slot in [128, 0, 64] {
            source
                .insert(source.entity_at(slot).unwrap(), Wide::Empty)
                .unwrap();
        }
        let bytes = source.save().unwrap();
        let mut destination = World::new(60, 0);
        destination.register::<Wide>().unwrap();
        let before = destination.save().unwrap();
        let (result, counts) = counting::measure(|| {
            destination.load_in(&bytes, Some(&crate::data::LoadBudget::new(65_536)), false)
        });
        assert!(result.is_err());
        assert!(
            counts.1 < 65_536,
            "page allocated before admission: {counts:?}"
        );
        assert_eq!(destination.save().unwrap(), before);
        destination.load(&bytes).unwrap();
        assert_eq!(destination.save().unwrap(), bytes);
    }
}
