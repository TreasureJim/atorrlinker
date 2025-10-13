# Atorrlinker Service

## Paths

/book - actions for books
/book/add - add book
    PARAMS
    title: String
    subtitle: String
    authors: [name: String]
    series: String
    series_sequence: String
    publish_year: uint16
    narrator: String

/plugin/list
/plugin/<plugin-name> - paths for all plugin related activities
    ./search - for searching through books using plugin
    ./search-
