# A small KDL v2 emitter.
#
# slowshell parses its configuration with `kdl = "6"` using KDL *v2*
# (`KdlDocument::parse_v2`), where booleans are `#true`/`#false` and a bare
# `true` is an identifier. Home Manager's own `lib.hm.generators.toKDL`
# emits KDL v1 (`true`/`false`), which slowshell rejects, so this file
# provides the handful of formatting helpers the module needs.
{ lib }:

let
  inherit (builtins)
    concatStringsSep
    isBool
    isFloat
    isInt
    isPath
    isString
    match
    replaceStrings
    toJSON
    ;

  # A KDL identifier: bare when it only uses the characters that never need
  # quoting, quoted otherwise.
  name = value: if match "[A-Za-z_][A-Za-z0-9_-]*" value != null then value else string value;

  # A KDL string literal. Single-line strings carry escapes rather than raw
  # newlines, because slowshell's parser rejects a literal newline inside a
  # `"..."` string.
  string =
    value:
    let
      escaped = replaceStrings [ "\\" "\"" "\n" "\r" "\t" ] [ "\\\\" "\\\"" "\\n" "\\r" "\\t" ] value;
    in
    "\"${escaped}\"";

  # Numbers keep their type: slowshell reads ints with `as_integer` and
  # floats with `as_float`, so an integer must not gain a decimal point.
  # `toJSON` is the only builtin that renders floats without the
  # six-decimal padding of `toString`.
  number =
    value:
    if isInt value then
      toString value
    else if isFloat value then
      let
        json = toJSON value;
      in
      if lib.hasInfix "." json || lib.hasInfix "e" json then json else "${json}.0"
    else
      throw "kdl: ${toJSON value} is not a number";

  literal =
    value:
    if isBool value then
      (if value then "#true" else "#false")
    else if isInt value || isFloat value then
      number value
    else if isString value then
      string value
    else if isPath value then
      string (toString value)
    else
      throw "kdl: cannot render a value of type ${builtins.typeOf value}";

  # A node: `name arg1 arg2 key=value { children }`. Children are either
  # nodes or already-rendered KDL strings, so a caller can chain `render`
  # calls without keeping the intermediate nodes around. Children are
  # rendered with two-space indentation and the closing brace is only
  # emitted when there is something to close.
  render =
    node:
    let
      head = concatStringsSep " " (
        [ (name node.name) ]
        ++ map literal (node.args or [ ])
        ++ map (key: "${name key}=${literal node.props.${key}}") (lib.attrNames (node.props or { }))
      );

      children = map (child: if isString child then child else render child) (node.children or [ ]);
    in
    if children == [ ] then head else "${head} {\n${indent (concatStringsSep "\n" children)}\n}";

  indent =
    text:
    concatStringsSep "\n" (
      map (line: if line == "" then "" else "  " + line) (lib.splitString "\n" text)
    );
in
{
  inherit
    literal
    name
    number
    render
    string
    ;
}
