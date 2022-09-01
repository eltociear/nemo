//! The data model.

use std::{
    collections::{HashMap, HashSet},
    fmt::Display,
    ops::Neg,
    path::{Path, PathBuf},
};

use crate::{
    generate_forwarder,
    io::parser::{ParseError, RuleParser},
    physical::{
        datatypes::Double,
        dictionary::{Dictionary, PrefixedStringDictionary},
    },
};

/// An identifier for, e.g., a Term or a Predicate.
#[derive(Debug, Eq, PartialEq, Hash, Copy, Clone, PartialOrd, Ord)]
pub struct Identifier(pub(crate) usize);

impl Identifier {
    /// Make the [`Identifier`] pretty-printable using the given
    /// [`PrefixedStringDictionary`].
    pub fn format<'a, 'b>(
        &'a self,
        dictionary: &'b PrefixedStringDictionary,
    ) -> PrintableIdentifier<'b>
    where
        'a: 'b,
    {
        PrintableIdentifier {
            identifier: self,
            dictionary,
        }
    }
}

/// A pretty-printable identifier that can be resolved using the dictionary.
#[derive(Debug)]
pub struct PrintableIdentifier<'a> {
    identifier: &'a Identifier,
    dictionary: &'a PrefixedStringDictionary,
}

impl Display for PrintableIdentifier<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let ident = self.identifier.0;
        write!(
            f,
            "{}",
            self.dictionary
                .entry(ident)
                .unwrap_or_else(|| format!("<unresolved identifier {ident}>"))
        )
    }
}

/// Terms occurring in programs.
#[derive(Debug, Eq, PartialEq, Copy, Clone, PartialOrd, Ord)]
pub enum Term {
    /// An (abstract) constant.
    Constant(Identifier),
    /// A (universally quantified) variable.
    Variable(Identifier),
    /// An existentially quantified variable.
    ExistentialVariable(Identifier),
    /// A numeric literal.
    NumericLiteral(NumericLiteral),
    /// An RDF literal.
    RdfLiteral(RdfLiteral),
}

impl Term {
    /// Check if the term is ground.
    pub fn is_ground(&self) -> bool {
        matches!(
            self,
            Self::Constant(_) | Self::NumericLiteral(_) | Self::RdfLiteral(_)
        )
    }
}

/// A numerical literal.
#[derive(Debug, Eq, PartialEq, Copy, Clone, PartialOrd, Ord)]
pub enum NumericLiteral {
    /// An integer literal.
    Integer(i64),
    /// A decimal literal.
    Decimal(i64, u64),
    /// A double literal.
    Double(Double),
}

/// An RDF literal.
#[derive(Debug, Eq, PartialEq, Hash, Copy, Clone, PartialOrd, Ord)]
pub enum RdfLiteral {
    /// A language string.
    LanguageString {
        /// The literal value.
        value: usize,
        /// The language tag.
        tag: usize,
    },
    /// A literal with a datatype.
    DatatypeValue {
        /// The literal value.
        value: usize,
        /// The datatype IRI.
        datatype: usize,
    },
}

/// An atom.
#[derive(Debug, Eq, PartialEq, Clone, PartialOrd, Ord)]
pub struct Atom {
    /// The predicate.
    predicate: Identifier,
    /// The terms.
    terms: Vec<Term>,
}

impl Atom {
    /// Construct a new Atom.
    pub fn new(predicate: Identifier, terms: Vec<Term>) -> Self {
        Self { predicate, terms }
    }

    /// Return the predicate [`Identifier`].
    #[must_use]
    pub fn predicate(&self) -> Identifier {
        self.predicate
    }

    /// Iterate over the terms in the atom.
    pub fn terms(&self) -> impl Iterator<Item = &Term> {
        self.terms.iter()
    }

    /// Iterate over all variables in the atom.
    pub fn variables(&self) -> impl Iterator<Item = &Term> {
        self.terms()
            .filter(|&term| matches!(term, Term::Variable(_) | Term::ExistentialVariable(_)))
    }

    /// Iterate over all universally quantified variables in the atom.
    pub fn universal_variables(&self) -> impl Iterator<Item = Identifier> + '_ {
        self.variables().filter_map(|&term| match term {
            Term::Variable(identifier) => Some(identifier),
            _ => None,
        })
    }

    /// Iterate over all existentially quantified variables in the atom.
    pub fn existential_variables(&self) -> impl Iterator<Item = Identifier> + '_ {
        self.variables().filter_map(|&term| match term {
            Term::ExistentialVariable(identifier) => Some(identifier),
            _ => None,
        })
    }
}

/// A literal.
#[derive(Debug, Eq, PartialEq, Clone, PartialOrd, Ord)]
pub enum Literal {
    /// A non-negated literal.
    Positive(Atom),
    /// A negated literal.
    Negative(Atom),
}

impl Literal {
    /// Check if the literal is positive.
    pub fn is_positive(&self) -> bool {
        matches!(self, Self::Positive(_))
    }

    /// Check if the literal is negative.
    pub fn is_negative(&self) -> bool {
        matches!(self, Self::Negative(_))
    }

    pub fn atom(&self) -> &Atom {
        match self {
            Self::Positive(atom) => atom,
            Self::Negative(atom) => atom,
        }
    }
}

impl Neg for Literal {
    type Output = Self;

    fn neg(self) -> Self::Output {
        match self {
            Literal::Positive(atom) => Self::Negative(atom),
            Literal::Negative(atom) => Self::Positive(atom),
        }
    }
}

generate_forwarder!(forward_to_atom; Positive, Negative);

impl Literal {
    /// Iterate over the terms in the literal.
    pub fn terms(&self) -> impl Iterator<Item = &Term> {
        forward_to_atom!(self, terms)
    }

    /// Iterate over the variables in the literal.
    pub fn variables(&self) -> impl Iterator<Item = &Term> {
        forward_to_atom!(self, variables)
    }

    /// Iterate over the universally quantified variables in the literal.
    pub fn universal_variables(&self) -> impl Iterator<Item = Identifier> + '_ {
        forward_to_atom!(self, universal_variables)
    }

    /// Iterate over the existentially quantified variables in the literal.
    pub fn existential_variables(&self) -> impl Iterator<Item = Identifier> + '_ {
        forward_to_atom!(self, existential_variables)
    }
}

/// A rule.
#[derive(Debug, Eq, PartialEq, Clone, PartialOrd, Ord)]
pub struct Rule {
    pub head: Vec<Atom>,
    pub body: Vec<Literal>,
}

impl Rule {
    /// Construct a new rule.
    pub fn new(head: Vec<Atom>, body: Vec<Literal>) -> Self {
        Self { head, body }
    }

    /// Construct a new rule, validating constraints on variable usage.
    pub(crate) fn new_validated(
        head: Vec<Atom>,
        body: Vec<Literal>,
        parser: &RuleParser,
    ) -> Result<Self, ParseError> {
        // Check if existential variables occur in the body.
        let existential_variables = body
            .iter()
            .flat_map(|literal| literal.existential_variables())
            .collect::<Vec<_>>();

        if !existential_variables.is_empty() {
            return Err(ParseError::BodyExistential(
                parser
                    .resolve_term(existential_variables.first().expect("is not empty here").0)
                    .expect("identifier has been parsed, so must be known"),
            ));
        }

        // Check if some variable in the body occurs only in negative literals.
        let (positive, negative): (Vec<_>, Vec<_>) = body
            .iter()
            .cloned()
            .partition(|literal| literal.is_positive());
        let negative_variables = negative
            .iter()
            .flat_map(|literal| literal.universal_variables())
            .collect::<HashSet<_>>();
        let positive_varibales = positive
            .iter()
            .flat_map(|literal| literal.universal_variables())
            .collect();
        let negative_variables = negative_variables
            .difference(&positive_varibales)
            .collect::<Vec<_>>();

        if !negative_variables.is_empty() {
            return Err(ParseError::UnsafeNegatedVariable(
                parser
                    .resolve_term(negative_variables.first().expect("is not empty here").0)
                    .expect("identifier has been parsed, so must be known"),
            ));
        }

        // Check if a variable occurs with both existential and universal quantification.
        let mut universal_variables = body
            .iter()
            .flat_map(|literal| literal.universal_variables())
            .collect::<HashSet<_>>();
        universal_variables.extend(head.iter().flat_map(|atom| atom.universal_variables()));
        let existential_variables = head
            .iter()
            .flat_map(|atom| atom.existential_variables())
            .collect();

        let common_variables = universal_variables
            .intersection(&existential_variables)
            .take(1)
            .collect::<Vec<_>>();

        if !common_variables.is_empty() {
            return Err(ParseError::BothQuantifiers(
                parser
                    .resolve_term(common_variables.first().expect("is not empty here").0)
                    .expect("identifier has been parsed, so must be known"),
            ));
        }

        Ok(Rule { head, body })
    }

    /// Iterate over the head atoms of the rule.
    pub fn head(&self) -> impl Iterator<Item = &Atom> {
        self.head.iter()
    }

    /// Iterate over the body literals of the rule.
    pub fn body(&self) -> impl Iterator<Item = &Literal> {
        self.body.iter()
    }
}

/// A (ground) fact.
#[derive(Debug, Eq, PartialEq, Clone, PartialOrd, Ord)]
pub struct Fact(pub(crate) Atom);

/// A statement that can occur in the program.
#[derive(Debug, Eq, PartialEq, Clone, PartialOrd, Ord)]
pub enum Statement {
    /// A fact.
    Fact(Fact),
    /// A rule.
    Rule(Rule),
}

/// A full program.
#[derive(Debug)]
pub struct Program {
    base: Option<usize>,
    prefixes: HashMap<String, usize>,
    sources: Vec<DataSourceDeclaration>,
    pub rules: Vec<Rule>,
    facts: Vec<Fact>,
}

impl Program {
    /// Construct a new program.
    pub fn new(
        base: Option<usize>,
        prefixes: HashMap<String, usize>,
        sources: Vec<DataSourceDeclaration>,
        rules: Vec<Rule>,
        facts: Vec<Fact>,
    ) -> Self {
        Self {
            base,
            prefixes,
            sources,
            rules,
            facts,
        }
    }

    /// Get the base IRI, if set.
    #[must_use]
    pub fn base(&self) -> Option<usize> {
        self.base
    }

    /// Iterate over all rules in the program.
    pub fn rules(&self) -> impl Iterator<Item = &Rule> {
        self.rules.iter()
    }

    /// Iterate over all facts in the program.
    pub fn facts(&self) -> impl Iterator<Item = &Fact> {
        self.facts.iter()
    }

    /// Iterate over all prefixes in the program.
    pub fn prefixes(&self) -> impl Iterator<Item = (&String, &usize)> {
        self.prefixes.iter()
    }

    /// Iterate over all data sources in the program.
    pub fn sources(&self) -> impl Iterator<Item = ((Identifier, usize), &DataSource)> {
        self.sources
            .iter()
            .map(|source| ((source.predicate, source.arity), &source.source))
    }

    /// Look up a given prefix.
    pub fn resolve_prefix(&self, tag: &str) -> Option<usize> {
        self.prefixes.get(tag).copied()
    }
}

/// A directive that can occur in the program.
#[derive(Debug)]
pub enum Directive {
    /// Import another source file.
    Import(Box<Path>),
    /// Import another source file with a relative path.
    ImportRelative(Box<Path>),
}

/// A SPARQL query.
#[derive(Debug, Clone)]
pub struct SparqlQuery {
    /// The SPARQL endpoint, should be an IRI.
    endpoint: String,
    /// The projection clause (the list of variables to select) for the SPARQL query.
    projection: String,
    /// The actual query.
    query: String,
}

impl SparqlQuery {
    /// Construct a new SPARQL query.
    pub fn new(endpoint: String, projection: String, query: String) -> Self {
        Self {
            endpoint,
            projection,
            query,
        }
    }

    /// Get the endpoint.
    #[must_use]
    pub fn endpoint(&self) -> &str {
        self.endpoint.as_ref()
    }

    /// Get the projection clause.
    #[must_use]
    pub fn projection(&self) -> &str {
        self.projection.as_ref()
    }

    /// Get the query part.
    #[must_use]
    pub fn query(&self) -> &str {
        self.query.as_ref()
    }

    /// Format a full SPARQL query.
    #[must_use]
    pub fn format_query(&self) -> String {
        format!("SELECT {} WHERE {{ {} }}", self.projection, self.query)
    }
}

/// An external data source.
#[derive(Debug, Clone)]
pub enum DataSource {
    /// A CSV file data source with the given path.
    CsvFile(Box<PathBuf>),
    /// An RDF file data source with the given path.
    RdfFile(Box<PathBuf>),
    /// A SPARQL query data source.
    SparqlQuery(Box<SparqlQuery>),
}

impl DataSource {
    /// Construct a new CSV file data source from a given path.
    pub fn csv_file(path: &str) -> Result<Self, ParseError> {
        Ok(Self::CsvFile(Box::new(PathBuf::from(path))))
    }

    /// Construct a new RDF file data source from a given path.
    pub fn rdf_file(path: &str) -> Result<Self, ParseError> {
        Ok(Self::RdfFile(Box::new(PathBuf::from(path))))
    }

    /// Construct a new SPARQL query data source from a given query.
    pub fn sparql_query(query: SparqlQuery) -> Result<Self, ParseError> {
        Ok(Self::SparqlQuery(Box::new(query)))
    }
}

/// A Data source declaration.
#[derive(Debug, Clone)]
pub struct DataSourceDeclaration {
    predicate: Identifier,
    arity: usize,
    source: DataSource,
}

impl DataSourceDeclaration {
    /// Construct a new data source declaration.
    pub fn new(predicate: Identifier, arity: usize, source: DataSource) -> Self {
        Self {
            predicate,
            arity,
            source,
        }
    }

    /// Construct a new data source declaration, validating constraints on, e.g., arity.
    pub(crate) fn new_validated(
        predicate: Identifier,
        arity: usize,
        source: DataSource,
        parser: &RuleParser,
    ) -> Result<Self, ParseError> {
        match source {
            DataSource::CsvFile(_) => (), // no validation for CSV files
            DataSource::RdfFile(ref path) => {
                if arity != 3 {
                    return Err(ParseError::RdfSourceInvalidArity(
                        parser
                            .resolve_term(predicate.0)
                            .expect("predicate has been parsed, must be known"),
                        path.to_str()
                            .unwrap_or("<path is invalid unicode>")
                            .to_owned(),
                        arity,
                    ));
                }
            }
            DataSource::SparqlQuery(ref query) => {
                let variables = query.projection().split(',').count();
                if variables != arity {
                    return Err(ParseError::SparqlSourceInvalidArity(
                        parser
                            .resolve_term(predicate.0)
                            .expect("predicate has been parsed, must be known"),
                        variables,
                        arity,
                    ));
                }
            }
        };

        Ok(Self {
            predicate,
            arity,
            source,
        })
    }
}
