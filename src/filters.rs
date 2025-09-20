use pyo3::prelude::*;

use crate::parser::Label;

// Define an enum for the different types of filters
#[pyclass]
#[derive(Debug, PartialEq, Clone)]
pub enum Filters {
    /// Basic filter: stores a nested list of filter expressions.
    Filter {
        filter: Vec<Vec<Vec<Label>>>,
    },
    /// NoOnlyFilter: filter + line string.
    NoOnlyFilter {
        filter: Vec<Vec<Vec<Label>>>,
        line: String,
    },
    /// OnlyFilter: inherits NoOnlyFilter.
    OnlyFilter {
        filter: Vec<Vec<Vec<Label>>>,
        line: String,
    },
    /// NoFilter: inherits NoOnlyFilter.
    NoFilter {
        filter: Vec<Vec<Vec<Label>>>,
        line: String,
    },
    /// JoinFilter: inherits NoOnlyFilter.
    JoinFilter {
        filter: Vec<Vec<Vec<Label>>>,
        line: String,
    },
    /// BlockFilter: just a boolean.
    BlockFilter {
        blocked: bool,
    },
    /// Condition: inherits NoFilter, adds content.
    Condition {
        filter: Vec<Vec<Vec<Label>>>,
        line: String,
    },
    /// NegativeCondition: inherits OnlyFilter, adds content.
    NegativeCondition {
        filter: Vec<Vec<Vec<Label>>>,
        line: String,
    },
}

#[pymethods]
impl Filters {
    /// Check if filter matches in context.
    pub fn match_ctx(&self, ctx: Vec<Label>) -> PyResult<bool> {
        match self {
            Filters::Filter { filter } | Filters::NoOnlyFilter { filter, .. } | Filters::OnlyFilter { filter, .. } | Filters::NoFilter { filter, .. } | Filters::JoinFilter { filter, .. } | Filters::Condition { filter, .. } | Filters::NegativeCondition { filter, .. } => {
                for word in filter {
                    let mut all_blocks_matched = true;
                    for block in word {
                        if Self::match_adjacent(block.clone(), ctx.clone()) != block.len() {
                            all_blocks_matched = false;
                            break;
                        }
                    }
                    if all_blocks_matched {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            _ => Ok(false)
        }
    }

    /// Check if filter might match in context, considering descendant labels.
    pub fn might_match(&self, ctx: Vec<Label>, descendant_labels: Vec<Label>) -> PyResult<bool> {
        match self {
            Filters::Filter { filter } | Filters::NoOnlyFilter { filter, .. } | Filters::OnlyFilter { filter, .. } | Filters::NoFilter { filter, .. } | Filters::Condition { filter, .. } | Filters::NegativeCondition { filter, .. } | Filters::JoinFilter { filter, .. } => {
                for word in filter {
                    let mut all_blocks_passed = true;
                    for block in word {
                        if !Self::might_match_adjacent(block.clone(), ctx.clone(), descendant_labels.clone()) {
                            all_blocks_passed = false;
                            break;
                        }
                    }
                    if all_blocks_passed {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            _ => Ok(false)
        }
    }

    pub fn is_irrelevant(&self, ctx: Vec<Label>, descendant_labels: Vec<Label>) -> PyResult<bool> {
        match self {
            // Matched in this tree.
            Filters::OnlyFilter { .. } => self.match_ctx(ctx),
            Filters::NoFilter { .. } => Ok(!self.might_match(ctx, descendant_labels)?),
            Filters::Condition { .. } => Ok(!self.might_match(ctx, descendant_labels)?),
            Filters::NegativeCondition { .. } => self.match_ctx(ctx),
            _ => Ok(false)
        }
    }

    pub fn requires_action(&self, ctx: Vec<Label>, descendant_labels: Vec<Label>) -> PyResult<bool> {
        match self {
            // Impossible to match in this tree.
            Filters::OnlyFilter { .. } => Ok(!self.might_match(ctx, descendant_labels)?),
            Filters::NoFilter { .. } => self.match_ctx(ctx),
            Filters::Condition { .. } => self.match_ctx(ctx),
            Filters::NegativeCondition { .. } => Ok(!self.might_match(ctx, descendant_labels)?),
            _ => Ok(false)
        }
    }

    pub fn might_pass(&self, failed_ctx: Vec<Label>, ctx: Vec<Label>, descendant_labels: Vec<Label>) -> PyResult<bool> {
        match self {
            Filters::OnlyFilter { filter, .. } | Filters::NegativeCondition { filter, .. } => {
                for word in filter {
                    for block in word {
                        if Self::match_adjacent(block.clone(), ctx.clone()) > Self::match_adjacent(block.clone(), failed_ctx.clone()) {
                            return self.might_match(ctx, descendant_labels);
                        }
                    }
                }
                Ok(false)
            }
            Filters::NoFilter { filter, .. } | Filters::Condition { filter, .. } => {
                for word in filter {
                    for block in word {
                        if Self::match_adjacent(block.clone(), ctx.clone()) < Self::match_adjacent(block.clone(), failed_ctx.clone()) {
                            return Ok(!self.match_ctx(ctx)?);
                        }
                    }
                }
                Ok(false)
            }
            _ => Ok(false)
        }
    }

    pub fn __str__(&self) -> PyResult<String> {
        match self {
            Filters::OnlyFilter { filter, .. } => Ok(format!("Only {:?}", filter)),
            Filters::NoFilter { filter, .. } => Ok(format!("No {:?}", filter)),
            Filters::JoinFilter { filter, .. } => Ok(format!("Join {:?}", filter)),
            Filters::Condition { filter, .. } => Ok(format!("Condition {:?}", filter)),
            Filters::NegativeCondition { filter, .. } => Ok(format!("NotCond {:?}", filter)),
            Filters::BlockFilter { blocked } => Ok(format!("BlockFilter blocked={}", blocked)),
            Filters::Filter { filter } => Ok(format!("Filter {:?}", filter)),
            Filters::NoOnlyFilter { filter, .. } => Ok(format!("NoOnlyFilter {:?}", filter)),
        }
    }

    pub fn __repr__(&self) -> PyResult<String> {
        self.__str__()
    }

    /// Try to match as many blocks as possible from context.
    #[staticmethod]
    fn match_adjacent(block: Vec<Label>, ctx: Vec<Label>) -> usize {
        if block.is_empty() || !ctx.contains(&block[0]) {
            return 0;
        }
        if block.len() == 1 {
            return 1;
        }
        if !ctx.contains(&block[1]) {
            return if ctx.last().is_some_and(|x| x == &block[0]) { 1 } else { 0 };
        }
        let mut k = 0;
        let mut i = ctx.iter().position(|x| x == &block[0]).unwrap_or(0);
        while i < ctx.len() {
            if k > 0 && ctx[i] != block[k] {
                i = i.saturating_sub(k - 1);
                k = 0;
            }
            if ctx[i] == block[k] {
                k += 1;
                if k >= block.len() {
                    break;
                }
                if !ctx.contains(&block[k]) {
                    break;
                }
            }
            i += 1;
        }
        k
    }

    /// Try to maybe match as many blocks as possible from context.
    #[staticmethod]
    fn might_match_adjacent(block: Vec<Label>, ctx: Vec<Label>, descendant_labels: Vec<Label>) -> bool {
        let matched = Self::match_adjacent(block.clone(), ctx.clone());
        for elem in block.iter().skip(matched) {
            if !descendant_labels.contains(elem) {
                return false;
            }
        }
        true
    }

    /*
    pub fn __eq__(&self, other: &Filters) -> PyResult<bool> {
        Ok(self == other)
    }
    def __eq__(self, o: "NoOnlyFilter") -> bool:
        if isinstance(o, self.__class__):
            if self.filter == o.filter:
                return True

        return False
    */
}
