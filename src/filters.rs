use std::iter::{once, Once, Chain};
use std::vec::IntoIter;

use pyo3::prelude::*;

use crate::tokens::Tokens;
use crate::parser::Label;
use crate::parser::ParserError;

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

    fn __eq__(&self, other: &Bound<'_, PyAny>) -> PyResult<bool> {
        match self {
            Filters::Filter { filter } | Filters::NoOnlyFilter { filter, .. } | Filters::OnlyFilter { filter, .. } | Filters::NoFilter { filter, .. } | Filters::JoinFilter { filter, .. } | Filters::Condition { filter, .. } | Filters::NegativeCondition { filter, .. } => {
                if other.hasattr("filter")? {
                    let other_filter: Vec<Vec<Vec<Label>>> = other.getattr("filter")?.extract()?;
                    Ok(filter.clone() == other_filter)
                } else {
                    Ok(false)
                }
            }
            _ => Ok(false)
        }
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

    /// Parse a filter from a list of tokens.
    /*
    More details on the syntax of the connectives for these filters:

    * ``,`` means ``OR``
    * ``..`` means ``AND``
    * ``.`` means ``IMMEDIATELY-FOLLOWED-BY``
    * ``(xx=yy)`` where ``xx=VARIANT_NAME`` and ``yy=VARIANT_VALUE``

    Example:

    ::

        qcow2..(guest_os=Fedora).14, RHEL.6..raw..boot, smp2..qcow2..migrate..ide

    means match all dicts whose names have:

    ::

        (qcow2 AND ((guest_os=Fedora) IMMEDIATELY-FOLLOWED-BY 14)) OR
        ((RHEL IMMEDIATELY-FOLLOWED-BY 6) AND raw AND boot) OR
        (smp2 AND qcow2 AND migrate AND ide)

    Note:

    * ``qcow2..Fedora.14`` is equivalent to ``Fedora.14..qcow2``.
    * ``qcow2..Fedora.14`` is not equivalent to ``qcow2..14.Fedora``.
    * ``ide, scsi`` is equivalent to ``scsi, ide``.
    */
    #[staticmethod]
    pub fn parse_filter(tokens: Vec<Tokens>, line: String, filename: String, linenum: isize) -> PyResult<Vec<Vec<Vec<Label>>>> {
        let mut or_filters = Vec::new();
        let mut and_filter = Vec::new();
        let mut con_filter = Vec::new();
        let mut dots = 1;

        let mut tokens_iter = tokens.into_iter().chain(once(Tokens::LEndL()));

        // Helper to get next non-whitespace token (used only when parsing inside parentheses)
        let next_nw = |iter: &mut Chain<IntoIter<Tokens>, Once<Tokens>>| -> Option<Tokens> {
            let mut token = iter.next();
            while matches!(token, Some(Tokens::LWhite(_))) {
                token = iter.next();
            }
            token
        };

        let mut first_token = true;
        let mut white_after_comma = false;
        while let Some(token) = tokens_iter.next() {
            match token {
                Tokens::LIdentifier(_) | Tokens::LLRBracket() => {
                    let label = if let Tokens::LLRBracket() = token {
                        // Handle (xxx=yyy) -- skip whitespace between internal tokens
                        let nw_token = next_nw(&mut tokens_iter);
                        let identifier = if let Some(Tokens::LIdentifier(s)) = nw_token {
                            s
                        } else {
                            return Err(PyErr::new::<ParserError, _>((
                                "Expected identifier after '('".to_string(),
                                Some(line),
                                Some(filename),
                                Some(linenum),
                            )));
                        };

                        match next_nw(&mut tokens_iter) {
                            Some(Tokens::LSet(_, _)) => {
                                // Handle (xxx = yyy) skipping whitespace between internal tokens
                                let nw_token = next_nw(&mut tokens_iter);
                                let value = match nw_token {
                                    Some(Tokens::LIdentifier(s)) | Some(Tokens::LString(s)) => s,
                                    _ => {
                                        return Err(PyErr::new::<ParserError, _>((
                                            "Expected value after '='".to_string(),
                                            Some(line),
                                            Some(filename),
                                            Some(linenum),
                                        )));
                                    }
                                };
                                match next_nw(&mut tokens_iter) {
                                    Some(Tokens::LRRBracket()) => Label::new(identifier, Some(value)),
                                    _ => {
                                        return Err(PyErr::new::<ParserError, _>((
                                            "Expected ')' after value".to_string(),
                                            Some(line),
                                            Some(filename),
                                            Some(linenum),
                                        )));
                                    }
                                }
                            }
                            Some(Tokens::LRRBracket()) => {
                                // Handle (xxx) skipping whitespace between internal tokens
                                Label::new(identifier, None)
                            }
                            _ => {
                                return Err(PyErr::new::<ParserError, _>((
                                    "Expected '=' or ')' after '( with format like (xxx=yyy) or (xxx)'".to_string(),
                                    Some(line),
                                    Some(filename),
                                    Some(linenum),
                                )));
                            }
                        }
                    } else if let Tokens::LIdentifier(s) = token {
                        // Handle other cases
                        Label::new(s, None)
                    } else {
                        return Err(PyErr::new::<ParserError, _>((
                            "Complex filter doesn't have format like (xxx=yyy) or (xxx)'".to_string(),
                            Some(line),
                            Some(filename),
                            Some(linenum),
                        )));
                    };

                    if dots == 1 {
                        con_filter.push(label);
                    } else if dots == 2 {
                        and_filter.push(con_filter);
                        con_filter = vec![label];
                    } else if dots == 0 || dots > 2{
                        return Err(PyErr::new::<ParserError, _>((
                            "Syntax Error: Expected '.' or '..' between identifiers".to_string(),
                            Some(line),
                            Some(filename),
                            Some(linenum),
                        )));
                    }

                    dots = 0;
                    white_after_comma = false;
                }
                Tokens::LDot() => {
                    // Handle xxx.xxxx or xxx..xxxx
                    if first_token {
                        return Err(PyErr::new::<ParserError, _>((
                            "Syntax Error: Filter cannot start with '.'".to_string(),
                            Some(line),
                            Some(filename),
                            Some(linenum),
                        )));
                    }
                    dots += 1;
                    white_after_comma = false;
                }
                Tokens::LWhite(_) if white_after_comma => {
                    continue;
                }
                Tokens::LComa() | Tokens::LWhite(_) => {
                    if matches!(token, Tokens::LComa()) && first_token {
                        return Err(PyErr::new::<ParserError, _>((
                            "Syntax Error: Filter cannot start with ','".to_string(),
                            Some(line),
                            Some(filename),
                            Some(linenum),
                        )));
                    }
                    if !white_after_comma && dots > 0 {
                        return Err(PyErr::new::<ParserError, _>((
                            "Syntax Error: Expected identifier between '.' and ','".to_string(),
                            Some(line),
                            Some(filename),
                            Some(linenum),
                        )));
                    }
                    if !con_filter.is_empty() {
                        and_filter.push(con_filter);
                        con_filter = Vec::new();
                    }
                    if !and_filter.is_empty() {
                        or_filters.push(and_filter);
                        and_filter = Vec::new();
                    }
                    dots = 1;
                    white_after_comma = true;
                }
                Tokens::LEndL() => {
                    break;
                }
                _ => {
                    return Err(PyErr::new::<ParserError, _>((
                        "Unexpected token in filter".to_string(),
                        Some(line),
                        Some(filename),
                        Some(linenum),
                    )));
                }
            }
            first_token = false;
        }

        if !con_filter.is_empty() {
            and_filter.push(con_filter);
        }
        if !and_filter.is_empty() {
            or_filters.push(and_filter);
        }

        Ok(or_filters)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_match_adjacent_basic() {
        let a = Label {
            name: "a".to_string(),
            var_name: None,
            long_name: "".to_string(),
            hash_val: 0,
            hash_var: None,
        };
        let b = Label {
            name: "b".to_string(),
            var_name: None,
            long_name: "".to_string(),
            hash_val: 1,
            hash_var: None,
        };
        let block = vec![a.clone()];
        let ctx = vec![a.clone(), b.clone()];
        assert_eq!(Filters::match_adjacent(block, ctx), 1);
    }

    //TODO: add local better isolating tests for functionality not exported to Python
}
