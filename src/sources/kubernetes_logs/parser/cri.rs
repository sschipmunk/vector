use crate::{
    event,
    transforms::{
        regex_parser::{RegexParser, RegexParserConfig},
        Transform,
    },
};

// TODO: validate against the k8s code.
// TODO: ensure partial event indicator is correctly set.
// TODO: add tests.
pub fn build() -> Box<dyn Transform> {
    let mut rp_config = RegexParserConfig::default();

    // message field
    rp_config.patterns = vec![
        r"^(?P<timestamp>.*) (?P<stream>(stdout|stderr)) (?P<multiline_tag>(P|F)) (?P<message>.*)$"
            .to_owned(),
    ];

    rp_config.types.insert(
        event::log_schema().timestamp_key().clone(),
        "timestamp|%+".to_owned(),
    );

    RegexParser::build(&rp_config).expect("regexp patterns are static, should never fail")
}
