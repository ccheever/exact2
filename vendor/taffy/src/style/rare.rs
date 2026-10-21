//! EXACT PATCH 62: fields that ordinary block and flex boxes leave at their defaults.
use super::*;
#[cfg(all(feature = "alloc", not(feature = "std")))]
use alloc::sync::Arc;
use core::ops::{Deref, DerefMut};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "std")]
use std::sync::Arc;

/// Grid and multi-column fields stored apart from the common style fields.
#[derive(Clone, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct RareStyle<S: CheapCloneStr = DefaultCheapStr> {
    /// Keeps the custom identifier type when grid layout is disabled.
    #[cfg_attr(feature = "serde", serde(skip))]
    pub dummy: core::marker::PhantomData<S>,
    // Grid container properties
    /// Defines the track sizing functions (heights) of the grid rows
    #[cfg(feature = "grid")]
    pub grid_template_rows: GridTrackVec<GridTemplateComponent<S>>,
    /// Defines the track sizing functions (widths) of the grid columns
    #[cfg(feature = "grid")]
    pub grid_template_columns: GridTrackVec<GridTemplateComponent<S>>,
    /// Defines the size of implicitly created rows
    #[cfg(feature = "grid")]
    pub grid_auto_rows: GridTrackVec<TrackSizingFunction>,
    /// Defined the size of implicitly created columns
    #[cfg(feature = "grid")]
    pub grid_auto_columns: GridTrackVec<TrackSizingFunction>,

    // Grid container named properties
    /// Defines the rectangular grid areas
    #[cfg(feature = "grid")]
    pub grid_template_areas: Option<GridTemplateAreas<S>>,
    /// The named lines between the columns
    #[cfg(feature = "grid")]
    pub grid_template_column_names: GridTrackVec<GridTrackVec<S>>,
    /// The named lines between the rows
    #[cfg(feature = "grid")]
    pub grid_template_row_names: GridTrackVec<GridTrackVec<S>>,

    // Grid child properties
    /// Defines which row in the grid the item should start and end at
    #[cfg(feature = "grid")]
    pub grid_row: Line<GridPlacement<S>>,
    /// Defines which column in the grid the item should start and end at
    #[cfg(feature = "grid")]
    pub grid_column: Line<GridPlacement<S>>,
    /// EXACT PATCH 27: a multi-column container's columns
    #[cfg(feature = "block_layout")]
    pub multicol: Option<Multicol>,
}

impl<S: CheapCloneStr> RareStyle<S> {
    /// Initial values, without allocating.
    pub const DEFAULT: Self = Self {
        dummy: core::marker::PhantomData,
        // Grid
        #[cfg(feature = "grid")]
        grid_template_rows: GridTrackVec::new(),
        #[cfg(feature = "grid")]
        grid_template_columns: GridTrackVec::new(),
        #[cfg(feature = "grid")]
        grid_template_areas: None,
        #[cfg(feature = "grid")]
        grid_template_column_names: GridTrackVec::new(),
        #[cfg(feature = "grid")]
        grid_template_row_names: GridTrackVec::new(),
        #[cfg(feature = "grid")]
        grid_auto_rows: GridTrackVec::new(),
        #[cfg(feature = "grid")]
        grid_auto_columns: GridTrackVec::new(),
        #[cfg(feature = "grid")]
        grid_row: Line { start: GridPlacement::<S>::Auto, end: GridPlacement::<S>::Auto },
        #[cfg(feature = "grid")]
        grid_column: Line { start: GridPlacement::<S>::Auto, end: GridPlacement::<S>::Auto },
        #[cfg(feature = "block_layout")]
        multicol: None,
    };

    #[cfg(any(feature = "alloc", feature = "std"))]
    const INITIAL: &'static Self = &Self::DEFAULT;
}

impl<S: CheapCloneStr> Default for RareStyle<S> {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Shared rare fields. Absence is the canonical initial representation;
/// mutable access may also leave an allocated group holding only defaults.
#[derive(Clone, Debug)]
pub struct Rare<S: CheapCloneStr = DefaultCheapStr>(
    #[cfg(any(feature = "alloc", feature = "std"))] Option<Arc<RareStyle<S>>>,
    #[cfg(not(any(feature = "alloc", feature = "std")))] RareStyle<S>,
);

impl<S: CheapCloneStr> Rare<S> {
    /// Initial fields without an allocation.
    #[cfg(any(feature = "alloc", feature = "std"))]
    pub const DEFAULT: Self = Self(None);
    /// Inline initial fields on hosts without an allocator.
    #[cfg(not(any(feature = "alloc", feature = "std")))]
    pub const DEFAULT: Self = Self(RareStyle::DEFAULT);

    /// Whether this style owns no rare-field allocation.
    #[cfg(any(feature = "alloc", feature = "std"))]
    pub fn is_initial(&self) -> bool {
        self.0.is_none()
    }
}

impl<S: CheapCloneStr> Default for Rare<S> {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl<S: CheapCloneStr> From<RareStyle<S>> for Rare<S> {
    fn from(fields: RareStyle<S>) -> Self {
        #[cfg(any(feature = "alloc", feature = "std"))]
        {
            Self((fields != RareStyle::DEFAULT).then(|| Arc::new(fields)))
        }
        #[cfg(not(any(feature = "alloc", feature = "std")))]
        {
            Self(fields)
        }
    }
}

impl<S: CheapCloneStr> Deref for Rare<S> {
    type Target = RareStyle<S>;
    fn deref(&self) -> &Self::Target {
        #[cfg(any(feature = "alloc", feature = "std"))]
        {
            self.0.as_deref().unwrap_or(RareStyle::INITIAL)
        }
        #[cfg(not(any(feature = "alloc", feature = "std")))]
        {
            &self.0
        }
    }
}

impl<S: CheapCloneStr> DerefMut for Rare<S> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        #[cfg(any(feature = "alloc", feature = "std"))]
        {
            Arc::make_mut(self.0.get_or_insert_with(|| Arc::new(RareStyle::DEFAULT)))
        }
        #[cfg(not(any(feature = "alloc", feature = "std")))]
        {
            &mut self.0
        }
    }
}

impl<S: CheapCloneStr + PartialEq> PartialEq for Rare<S> {
    fn eq(&self, other: &Self) -> bool {
        #[cfg(any(feature = "alloc", feature = "std"))]
        if self.0.is_none() && other.0.is_none() {
            return true;
        }
        // Compare values even for a shared allocation: floats may contain NaN.
        **self == **other
    }
}

#[cfg(feature = "serde")]
impl<S: CheapCloneStr + Serialize> Serialize for Rare<S> {
    fn serialize<T: serde::Serializer>(&self, serializer: T) -> Result<T::Ok, T::Error> {
        (**self).serialize(serializer)
    }
}

#[cfg(feature = "serde")]
impl<'de, S: CheapCloneStr + Deserialize<'de>> Deserialize<'de> for Rare<S> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        RareStyle::deserialize(deserializer).map(Self::from)
    }
}

#[cfg(all(test, feature = "grid", feature = "block_layout"))]
mod tests {
    use super::*;
    use crate::style_helpers::{length, line};

    #[test]
    fn rare_defaults_and_clone_mutation_preserve_style_values() {
        let initial: Style = Style::DEFAULT;
        assert!(initial.rare.is_initial());
        assert!(Style::<DefaultCheapStr>::default().rare.is_initial());
        assert!(Rare::<DefaultCheapStr>::from(RareStyle::DEFAULT).is_initial());
        assert_eq!(*initial.rare, RareStyle::DEFAULT);
        assert_eq!(GridContainerStyle::grid_template_rows(&initial).unwrap().count(), 0);
        assert_eq!(GridContainerStyle::grid_auto_columns(&initial).count(), 0);
        assert_eq!(GridItemStyle::grid_row(&initial), Line::auto());
        assert_eq!(BlockContainerStyle::multicol(&initial), None);

        let mut changed = initial.clone();
        changed.rare.grid_template_columns.push(length(80.0));
        changed.rare.grid_column.start = line(2);
        changed.rare.multicol =
            Some(Multicol { count: Some(2), width: None, gap: 12.0, used_height: Some(100.0), overflow: None });
        let mut copy = changed.clone();
        assert!(Arc::ptr_eq(changed.rare.0.as_ref().unwrap(), copy.rare.0.as_ref().unwrap()));
        copy.rare.grid_template_columns[0] = length(40.0);
        copy.rare.grid_column.start = line(3);
        copy.rare.multicol.as_mut().unwrap().count = Some(3);
        assert_eq!(BlockContainerStyle::multicol(&changed).unwrap().count, Some(2));
        assert_eq!(changed.rare.grid_template_columns, vec![length(80.0)]);
        assert_eq!(GridItemStyle::grid_column(&changed).start, line(2));
        assert_ne!(changed, copy);
        assert_eq!(*initial.rare, RareStyle::DEFAULT);

        *copy.rare = RareStyle::DEFAULT;
        assert!(!copy.rare.is_initial());
        assert_eq!(initial, copy);
        assert_eq!(copy, initial);
        assert_eq!(copy.clone(), initial);
        assert!(Rare::from((*copy.rare).clone()).is_initial());
    }

    #[test]
    fn sharing_does_not_make_nan_fields_equal() {
        let mut style: Style = Style::DEFAULT;
        style.rare.multicol =
            Some(Multicol { count: Some(2), width: None, gap: f32::NAN, used_height: None, overflow: None });
        assert_ne!(style, style.clone());
    }

    #[test]
    #[cfg(feature = "serde")]
    fn serde_keeps_flat_fields_and_canonicalizes_defaults() {
        let mut style: Style = Style::DEFAULT;
        style.rare.grid_column.start = line(2);
        let json = serde_json::to_value(&style).unwrap();
        assert!(json.get("rare").is_none());
        assert!(json.get("grid_column").is_some());
        assert_eq!(style, serde_json::from_value(json).unwrap());
        *style.rare = RareStyle::DEFAULT;
        let decoded: Style = serde_json::from_value(serde_json::to_value(&style).unwrap()).unwrap();
        assert_eq!(decoded, style);
        assert!(decoded.rare.is_initial());
    }
}
