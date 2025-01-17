//! Reading of RDF 1.1 triples files (N-Triples, Turtle, RDF/XML)
use std::io::{BufRead, BufReader};

use nemo_physical::{
    builder_proxy::{ColumnBuilderProxy, PhysicalBuilderProxyEnum},
    error::ReadingError,
    table_reader::{Resource, TableReader},
};
use oxiri::Iri;
use rio_api::{
    model::{BlankNode, NamedNode, Subject, Triple},
    parser::TriplesParser,
};
use rio_turtle::{NTriplesParser, TurtleParser};
use rio_xml::RdfXmlParser;

use crate::{
    builder_proxy::LogicalColumnBuilderProxyT,
    io::{formats::PROGRESS_NOTIFY_INCREMENT, resource_providers::ResourceProviders},
    model::{types::primitive_types::PrimitiveType, InvalidRdfLiteral, RdfFile, RdfLiteral, Term},
};

impl From<NamedNode<'_>> for Term {
    fn from(value: NamedNode) -> Self {
        Term::Constant(value.iri.to_string().into())
    }
}

impl From<BlankNode<'_>> for Term {
    fn from(value: BlankNode) -> Self {
        Term::Constant(value.to_string().into())
    }
}

impl TryFrom<rio_api::model::Literal<'_>> for Term {
    type Error = InvalidRdfLiteral;

    fn try_from(value: rio_api::model::Literal<'_>) -> Result<Self, Self::Error> {
        match value {
            rio_api::model::Literal::Simple { value } => Ok(Term::StringLiteral(value.to_string())),
            rio_api::model::Literal::LanguageTaggedString { value, language } => {
                Term::try_from(RdfLiteral::LanguageString {
                    value: value.to_string(),
                    tag: language.to_string(),
                })
            }
            rio_api::model::Literal::Typed { value, datatype } => {
                Term::try_from(RdfLiteral::DatatypeValue {
                    value: value.to_string(),
                    datatype: datatype.iri.to_string(),
                })
            }
        }
    }
}

impl TryFrom<Subject<'_>> for Term {
    type Error = ReadingError;

    fn try_from(value: Subject<'_>) -> Result<Self, Self::Error> {
        match value {
            Subject::NamedNode(nn) => Ok(nn.into()),
            Subject::BlankNode(bn) => Ok(bn.into()),
            Subject::Triple(_t) => Err(ReadingError::RdfStarUnsupported),
        }
    }
}

impl TryFrom<rio_api::model::Term<'_>> for Term {
    type Error = ReadingError;

    fn try_from(value: rio_api::model::Term<'_>) -> Result<Self, Self::Error> {
        match value {
            rio_api::model::Term::NamedNode(nn) => Ok(nn.into()),
            rio_api::model::Term::BlankNode(bn) => Ok(bn.into()),
            rio_api::model::Term::Literal(lit) => lit.try_into().map_err(Into::into),
            rio_api::model::Term::Triple(_t) => Err(ReadingError::RdfStarUnsupported),
        }
    }
}

/// A [`TableReader`] for RDF 1.1 files containing triples.
#[derive(Debug, Clone)]
pub struct RDFTriplesReader {
    resource_providers: ResourceProviders,
    resource: Resource,
    base: Option<Iri<String>>,
    logical_types: Vec<PrimitiveType>,
}

impl RDFTriplesReader {
    /// Create a new [`RDFTriplesReader`]
    pub fn new(
        resource_providers: ResourceProviders,
        rdf_file: &RdfFile,
        logical_types: Vec<PrimitiveType>,
    ) -> Self {
        Self {
            resource_providers,
            resource: rdf_file.resource.clone(),
            base: rdf_file
                .base
                .as_ref()
                .cloned()
                .map(|iri| Iri::parse(iri).expect("should be a valid IRI.")),
            logical_types,
        }
    }

    fn read_with_buf_reader<'a, 'b, Reader, Parser, MakeParser>(
        &self,
        physical_builder_proxies: &'b mut [PhysicalBuilderProxyEnum<'a>],
        reader: &'b mut Reader,
        make_parser: MakeParser,
    ) -> Result<(), ReadingError>
    where
        'a: 'b,
        Reader: BufRead,
        Parser: TriplesParser,
        MakeParser: FnOnce(&'b mut Reader) -> Parser,
        ReadingError: From<<Parser as TriplesParser>::Error>,
    {
        let mut builders = physical_builder_proxies
            .iter_mut()
            .zip(self.logical_types.clone())
            .map(|(bp, lt)| lt.wrap_physical_column_builder(bp))
            .collect::<Vec<_>>();

        assert!(builders.len() == 3);

        let mut triples = 0;
        let mut on_triple = |triple: Triple| {
            let subject: Term = triple.subject.try_into()?;
            let predicate: Term = triple.predicate.into();
            let object: Term = triple.object.try_into()?;

            <LogicalColumnBuilderProxyT as ColumnBuilderProxy<Term>>::add(
                &mut builders[0],
                subject,
            )?;
            if let Err(e) = <LogicalColumnBuilderProxyT as ColumnBuilderProxy<Term>>::add(
                &mut builders[1],
                predicate,
            ) {
                <LogicalColumnBuilderProxyT as ColumnBuilderProxy<Term>>::forget(&mut builders[0]);
                return Err(e);
            }
            if let Err(e) = <LogicalColumnBuilderProxyT as ColumnBuilderProxy<Term>>::add(
                &mut builders[2],
                object,
            ) {
                <LogicalColumnBuilderProxyT as ColumnBuilderProxy<Term>>::forget(&mut builders[0]);
                <LogicalColumnBuilderProxyT as ColumnBuilderProxy<Term>>::forget(&mut builders[1]);
                return Err(e);
            }

            triples += 1;
            if triples % PROGRESS_NOTIFY_INCREMENT == 0 {
                log::info!("Loading: processed {triples} triples")
            }

            Ok::<_, ReadingError>(())
        };

        let mut parser = make_parser(reader);

        while !parser.is_end() {
            if let Err(e) = parser.parse_step(&mut on_triple) {
                log::info!("Ignoring malformed triple: {e}");
            }
        }

        log::info!("Finished loading: processed {triples} triples");

        Ok(())
    }
}

impl TableReader for RDFTriplesReader {
    fn read_into_builder_proxies<'a: 'b, 'b>(
        self: Box<Self>,
        builder_proxies: &'b mut Vec<PhysicalBuilderProxyEnum<'a>>,
    ) -> Result<(), ReadingError> {
        let reader = self
            .resource_providers
            .open_resource(&self.resource, true)?;

        let mut reader = BufReader::new(reader);

        if self.resource.ends_with(".ttl.gz") || self.resource.ends_with(".ttl") {
            self.read_with_buf_reader(builder_proxies, &mut reader, |reader| {
                TurtleParser::new(reader, self.base.clone())
            })
        } else if self.resource.ends_with(".rdf.gz") || self.resource.ends_with(".rdf") {
            self.read_with_buf_reader(builder_proxies, &mut reader, |reader| {
                RdfXmlParser::new(reader, self.base.clone())
            })
        } else {
            self.read_with_buf_reader(builder_proxies, &mut reader, NTriplesParser::new)
        }
    }
}

#[cfg(test)]
mod test {
    use std::cell::RefCell;

    use nemo_physical::{
        builder_proxy::{PhysicalColumnBuilderProxy, PhysicalStringColumnBuilderProxy},
        datatypes::data_value::{DataValueIteratorT, PhysicalString},
        dictionary::{Dictionary, PrefixedStringDictionary},
    };
    use rio_turtle::TurtleParser;
    use test_log::test;

    use super::*;

    #[test]
    fn example_1() {
        macro_rules! parse_example_with_rdf_parser {
            ($make_parser:expr) => {
                let mut data = r#"<http://one.example/subject1> <http://one.example/predicate1> <http://one.example/object1> . # comments here
                      # or on a line by themselves
                      _:subject1 <http://an.example/predicate1> "object1" .
                      _:subject2 <http://an.example/predicate2> "object2" .
                      "#.as_bytes();

                let dict = RefCell::new(PrefixedStringDictionary::default());
                let mut builders = vec![
                    PhysicalBuilderProxyEnum::String(PhysicalStringColumnBuilderProxy::new(&dict)),
                    PhysicalBuilderProxyEnum::String(PhysicalStringColumnBuilderProxy::new(&dict)),
                    PhysicalBuilderProxyEnum::String(PhysicalStringColumnBuilderProxy::new(&dict)),
                ];
                let reader = RDFTriplesReader::new(ResourceProviders::empty(), &RdfFile::new("", None), vec![PrimitiveType::Any, PrimitiveType::Any, PrimitiveType::Any]);

                let result = reader.read_with_buf_reader(&mut builders, &mut data, $make_parser);
                assert!(result.is_ok());

                let columns = builders
                    .into_iter()
                    .map(|builder| match builder {
                        PhysicalBuilderProxyEnum::String(b) => b.finalize(),
                        _ => unreachable!("only string columns here"),
                    })
                    .collect::<Vec<_>>();

                log::debug!("columns: {columns:?}");
                let triples = (0..=2)
                    .map(|idx| {
                        columns
                            .iter()
                            .map(|column| {
                                column
                                    .get(idx)
                                    .and_then(|value| value.try_into().ok())
                                    .and_then(|u64: u64| usize::try_from(u64).ok())
                                    .and_then(|usize| dict.borrow_mut().entry(usize))
                                    .unwrap()
                            })
                            .map(PhysicalString::from)
                            .collect::<Vec<_>>()
                    })
                    .collect::<Vec<_>>();
                log::debug!("triple: {triples:?}");
                for (value, expected) in PrimitiveType::Any.serialize_output(DataValueIteratorT::String(Box::new(triples[0].iter().cloned()))).zip(vec!["http://one.example/subject1", "http://one.example/predicate1", "http://one.example/object1"]) {
                    assert_eq!(value, expected);
                }
                for (value, expected) in PrimitiveType::Any.serialize_output(DataValueIteratorT::String(Box::new(triples[1].iter().cloned()))).zip(vec!["_:subject1", "http://an.example/predicate1", r#""object1""#]) {
                    assert_eq!(value, expected);
                }
                for (value, expected) in PrimitiveType::Any.serialize_output(DataValueIteratorT::String(Box::new(triples[2].iter().cloned()))).zip(vec!["_:subject2", "http://an.example/predicate2", r#""object2""#]) {
                    assert_eq!(value, expected);
                }
            };
        }

        parse_example_with_rdf_parser!(NTriplesParser::new);
        parse_example_with_rdf_parser!(|reader| TurtleParser::new(reader, None));
    }

    #[test]
    fn rollback() {
        let mut data = r#"<http://example.org/> <http://example.org/> <http://example.org/> .
                          malformed <http://example.org/> <http://example.org/>
                          <http://example.org/> malformed <http://example.org/> .
                          <http://example.org/> <http://example.org/> malformed .
                          <http://example.org/> <http://example.org/> "123"^^<http://www.w3.org/2001/XMLSchema#integer> .
                          <http://example.org/> <http://example.org/> "123.45"^^<http://www.w3.org/2001/XMLSchema#integer> .
                          <http://example.org/> <http://example.org/> "123.45"^^<http://www.w3.org/2001/XMLSchema#decimal> .
                          <http://example.org/> <http://example.org/> "123.45a"^^<http://www.w3.org/2001/XMLSchema#decimal> .
                          <https://example.org/> <https://example.org/> <https://example.org/> .
                      "#
        .as_bytes();

        let dict = RefCell::new(PrefixedStringDictionary::default());
        let mut builders = vec![
            PhysicalBuilderProxyEnum::String(PhysicalStringColumnBuilderProxy::new(&dict)),
            PhysicalBuilderProxyEnum::String(PhysicalStringColumnBuilderProxy::new(&dict)),
            PhysicalBuilderProxyEnum::String(PhysicalStringColumnBuilderProxy::new(&dict)),
        ];
        let reader = RDFTriplesReader::new(
            ResourceProviders::empty(),
            &RdfFile::new("", None),
            vec![PrimitiveType::Any, PrimitiveType::Any, PrimitiveType::Any],
        );

        let result = reader.read_with_buf_reader(&mut builders, &mut data, NTriplesParser::new);
        assert!(result.is_ok());

        let columns = builders
            .into_iter()
            .map(|builder| match builder {
                PhysicalBuilderProxyEnum::String(b) => b.finalize(),
                _ => unreachable!("only string columns here"),
            })
            .collect::<Vec<_>>();

        assert_eq!(columns.len(), 3);
        assert_eq!(columns[0].len(), 4);
        assert_eq!(columns[1].len(), 4);
        assert_eq!(columns[2].len(), 4);
    }
}
