use std::{fmt::Debug, sync::Arc};

use formality_types::grammar::{AtomicPredicate, Invariant, ProgramClause, APR};

use crate::Env;

pub mod mock;

pub trait Database: Debug {
    /// Returns true if the solver should not attempt to prove `apr` but instead should return ambiguous.
    /// Keep in mind that `apr` may contain unrefreshed inference variables.
    fn force_ambiguous(&self, env: &Env, apr: &APR) -> bool;

    /// Returns a superset of the program clauses that can be used to prove `predicate` is true.
    /// These are derived from the program source (but may also include hard-coded rules).
    fn program_clauses(&self, predicate: &AtomicPredicate) -> Vec<ProgramClause>;

    /// Returns
    fn invariants_for_apr(&self, apr: &APR) -> Vec<Invariant>;
}

/// A handle to the database. Only equal to itself.
#[derive(Clone)]
pub struct Db {
    db: Arc<dyn Database + Send>,
    solver_config: SolverConfiguration,
}

#[derive(Copy, Clone, PartialOrd, Ord, PartialEq, Eq, Debug, Hash)]
pub enum SolverConfiguration {
    Cosld,
}

impl Db {
    pub fn new(db: impl Database + Send + 'static) -> Self {
        Self {
            db: Arc::new(db),
            solver_config: SolverConfiguration::Cosld,
        }
    }

    pub fn solver_config(&self) -> SolverConfiguration {
        self.solver_config
    }

    fn fields(&self) -> (*const (dyn Database + Send), &SolverConfiguration) {
        let Db { db, solver_config } = self;
        (Arc::as_ptr(db), solver_config)
    }
}

impl Debug for Db {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Db {
            db: _,
            solver_config,
        } = self;
        f.debug_struct("Db")
            .field("solver_config", solver_config)
            .finish()
    }
}

impl PartialEq for Db {
    fn eq(&self, other: &Self) -> bool {
        self.fields().eq(&other.fields())
    }
}

impl Eq for Db {}

impl PartialOrd for Db {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.fields().partial_cmp(&other.fields())
    }
}

impl Ord for Db {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.fields().cmp(&other.fields())
    }
}

impl std::hash::Hash for Db {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.fields().hash(state)
    }
}

impl Database for Db {
    fn invariants_for_apr(&self, apr: &APR) -> Vec<Invariant> {
        self.db.invariants_for_apr(apr)
    }

    fn program_clauses(&self, predicate: &AtomicPredicate) -> Vec<ProgramClause> {
        self.db.program_clauses(predicate)
    }

    fn force_ambiguous(&self, env: &Env, apr: &APR) -> bool {
        self.db.force_ambiguous(env, apr)
    }
}