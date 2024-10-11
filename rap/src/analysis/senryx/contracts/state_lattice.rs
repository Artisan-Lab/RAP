use super::abstract_state::*;

pub trait Lattice {
    fn join(&self, other: Self) -> Self;
    fn meet(&self, other: Self) -> Self;
    fn less_than(&self, other: Self) -> bool;
    fn equal(&self, other: Self) -> bool;
}

impl Lattice for StateType {
    fn join(&self, other: Self) -> Self {
        match self {
            &StateType::AllocatedState(a) => match other {
                StateType::AllocatedState(b) => StateType::AllocatedState(a.join(b)),
                _ => panic!("Incompatible types"),
            },
            &StateType::AlignState(a) => match other {
                StateType::AlignState(b) => StateType::AlignState(a.join(b)),
                _ => panic!("Incompatible types"),
            },
        }
    }

    fn meet(&self, other: Self) -> Self {
        match self {
            &StateType::AllocatedState(a) => match other {
                StateType::AllocatedState(b) => StateType::AllocatedState(a.meet(b)),
                _ => panic!("Incompatible types"),
            },
            &StateType::AlignState(a) => match other {
                StateType::AlignState(b) => StateType::AlignState(a.meet(b)),
                _ => panic!("Incompatible types"),
            },
        }
    }

    fn less_than(&self, other: Self) -> bool {
        match self {
            &StateType::AllocatedState(a) => match other {
                StateType::AllocatedState(b) => a.less_than(b),
                _ => panic!("Incompatible types"),
            },
            &StateType::AlignState(a) => match other {
                StateType::AlignState(b) => a.less_than(b),
                _ => panic!("Incompatible types"),
            },
        }
    }

    fn equal(&self, other: Self) -> bool {
        match self {
            &StateType::AllocatedState(a) => match other {
                StateType::AllocatedState(b) => a.equal(b),
                _ => panic!("Incompatible types"),
            },
            &StateType::AlignState(a) => match other {
                StateType::AlignState(b) => a.equal(b),
                _ => panic!("Incompatible types"),
            },
        }
    }
}

impl Lattice for AllocatedState {
    fn join(&self, other: Self) -> Self {
        match (*self, other) {
            (AllocatedState::Bottom, _) => other,
            (_, AllocatedState::Bottom) => *self,
            (AllocatedState::Top, _) | (_, AllocatedState::Top) => AllocatedState::Top,
            (AllocatedState::Borrowed, AllocatedState::Moved) | (AllocatedState::Moved, AllocatedState::Borrowed) => AllocatedState::Top,
            (AllocatedState::Alloc, AllocatedState::SpecificAlloc) | (AllocatedState::SpecificAlloc, AllocatedState::Alloc) => AllocatedState::SpecificAlloc,
            (state1, state2) if state1 == state2 => state1,
            (AllocatedState::Alloc, AllocatedState::Borrowed) | (AllocatedState::Borrowed, AllocatedState::Alloc) => AllocatedState::Borrowed,
            (AllocatedState::Alloc, AllocatedState::Moved) | (AllocatedState::Moved, AllocatedState::Alloc) => AllocatedState::Moved,
            (AllocatedState::SpecificAlloc, AllocatedState::Borrowed) | (AllocatedState::Borrowed, AllocatedState::SpecificAlloc) => AllocatedState::Borrowed,
            (AllocatedState::Moved, AllocatedState::SpecificAlloc) | (AllocatedState::SpecificAlloc, AllocatedState::Moved) => AllocatedState::Moved,
            _ => AllocatedState::Top,
        }
    }

    fn meet(&self, other: Self) -> Self {
        match (*self, other) {
            (AllocatedState::Top, _) => other,
            (_, AllocatedState::Top) => *self,
            (AllocatedState::Bottom, _) | (_, AllocatedState::Bottom) => AllocatedState::Bottom,
            (AllocatedState::Borrowed, AllocatedState::Moved)
            | (AllocatedState::Moved, AllocatedState::Borrowed) => AllocatedState::Bottom,
            (AllocatedState::Alloc, AllocatedState::SpecificAlloc) | (AllocatedState::SpecificAlloc, AllocatedState::Alloc) => AllocatedState::Alloc,
            (state1, state2) if state1 == state2 => state1,
            (AllocatedState::Alloc, AllocatedState::Borrowed) | (AllocatedState::Borrowed, AllocatedState::Alloc) => AllocatedState::Alloc,
            (AllocatedState::SpecificAlloc, AllocatedState::Borrowed) | (AllocatedState::Borrowed, AllocatedState::SpecificAlloc) => AllocatedState::SpecificAlloc,
            (AllocatedState::Moved, AllocatedState::SpecificAlloc) | (AllocatedState::SpecificAlloc, AllocatedState::Moved) => AllocatedState::SpecificAlloc,
            _ => AllocatedState::Bottom,
        }
    }

    fn less_than(&self, other: Self) -> bool {
        match (*self, other) {
            (AllocatedState::Bottom, _) | (_, AllocatedState::Top) => true,
            (AllocatedState::Alloc, AllocatedState::Borrowed) => true,
            (AllocatedState::Alloc, AllocatedState::SpecificAlloc) => true,
            (AllocatedState::Alloc, AllocatedState::Moved) => true,
            (AllocatedState::SpecificAlloc, AllocatedState::Borrowed) => true,
            (AllocatedState::SpecificAlloc, AllocatedState::Moved) => true,
            _ => false,
        }
    }

    fn equal(&self, other: Self) -> bool {
        *self == other
    }
}

impl Lattice for AlignState {
    fn join(&self, other: Self) -> Self {
        match (self, other) {
            (AlignState::Aligned, _) => AlignState::Aligned,
            (AlignState::Big2SmallCast, AlignState::Big2SmallCast) => AlignState::Big2SmallCast,
            (AlignState::Big2SmallCast, AlignState::Small2BigCast) => AlignState::Big2SmallCast,
            (AlignState::Big2SmallCast, AlignState::Aligned) => AlignState::Aligned,
            (AlignState::Small2BigCast, _) => other,
        }
    }

    fn meet(&self, other: Self) -> Self {
        match (self, other) {
            (AlignState::Aligned, _) => other,
            (AlignState::Big2SmallCast, AlignState::Big2SmallCast) => AlignState::Big2SmallCast,
            (AlignState::Big2SmallCast, AlignState::Small2BigCast) => AlignState::Small2BigCast,
            (AlignState::Big2SmallCast, AlignState::Aligned) => AlignState::Big2SmallCast,
            (AlignState::Small2BigCast, _) => AlignState::Small2BigCast,
        }
    }

    fn less_than(&self, other: Self) -> bool {
        match (self, other) {
            (_, AlignState::Aligned) => true,
            (AlignState::Small2BigCast, AlignState::Big2SmallCast) => true,
            _ => false,
        }
    }

    fn equal(&self, other: Self) -> bool {
        *self == other
    }
}

impl Lattice for InitState {
    fn join(&self, other: Self) -> Self {
        match (self, other) {
            (InitState::FullyInitialized, _) => InitState::FullyInitialized,
            (_, InitState::FullyInitialized) => InitState::FullyInitialized,
            _ => InitState::PartlyInitialized,
        }
    }

    fn meet(&self, other: Self) -> Self {
        match (self, other) {
            (InitState::FullyInitialized, _) => other,
            (_, InitState::FullyInitialized) => *self,
            _ => InitState::PartlyInitialized,
        }
    }

    fn less_than(&self, other: Self) -> bool {
        match (self, other) {
            (InitState::FullyInitialized, InitState::FullyInitialized) => true,
            (InitState::PartlyInitialized, _) => true,
            _ => false,
        }
    }

    fn equal(&self, other: Self) -> bool {
        *self == other
    }
}