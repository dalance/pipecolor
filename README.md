# pipecolor
A terminal filter to colorize output

[![Build Status](https://travis-ci.org/dalance/pipecolor.svg?branch=master)](https://travis-ci.org/dalance/pipecolor)
[![Crates.io](https://img.shields.io/crates/v/pipecolor.svg)](https://crates.io/crates/pipecolor)
[![codecov](https://codecov.io/gh/dalance/pipecolor/branch/master/graph/badge.svg)](https://codecov.io/gh/dalance/pipecolor)

## Description

**pipecolor** is a terminal filter to colorize output.
You can customize the colorize rule by regular expression.

## Install
Download from [release page](https://github.com/dalance/pipecolor/releases/latest), and extract to the directory in PATH.

Alternatively you can install by [cargo](https://crates.io).

```
cargo install pipecolor
```

Put the colorize rule file to `~/.pipecolor.toml`.

`sample/pipecolor.toml` in this repository is an example.

## Usage

**pipecolor** can receive input through pipe, and colorize the output.

```
$ cat sample/access_log | pipecolor -c ./sample/pipecolor.toml
```

<a><img src="https://rawgit.com/dalance/pipecolor/master/sample/access_log.svg"/></a>

Filenames can be specified.

```
$ pipecolor -c ./sample/pipecolor.toml sample/maillog
```

<a><img src="https://rawgit.com/dalance/pipecolor/master/sample/maillog.svg"/></a>

### Colorize rule

See the example rule `sample/pipecolor.toml`.

```
[[formats]]
    name = "httpd"
    pat  = "^(.*?) .*? .*? \\[(.*?)\\] \"(.*?)\" (.*?) .*? \"(.*?)\" \"(.*?)\""
    [[formats.lines]]
        pat  = "^(.*?) .*? .*? \\[(.*?)\\] \".*?\" .*? .*? \".*?\" \"(.*?)\""
        colors = ["White", "LightGreen", "LightBlue", "Green"]
        [[formats.lines.tokens]]
            pat   = "GET"
            colors = ["LightCyan"]
        [[formats.lines.tokens]]
            pat   = "POST"
            colors = ["LightYellow"]
        [[formats.lines.tokens]]
            pat   = "HEAD"
            colors = ["LightMagenta"]
```

`formats.pat` is a regular expression to specify a format start.
By this, some colorize rules can be switched dynamically.

```
[[formats]]
    name = "tool A"
    pat  = "^tool A started"
    ...

[[formats]]
    name = "tool B"
    pat  = "^tool B started"
    ...
```

`formats.lines.pat` is a regular expression to specify colorize lines.
If the expression is matched, the matched line is colorize to colors specified by `formats.lines.colors`.

`formats.lines.colors` is an array of colors, the first color is used to colorize the whole line.
The rest colors are used to colorize the captured group in the expression.
In the example, the whole line is colorized to `White`, the first group captured by `(.*?)` is colorized to `LightGreen`.

`formats.lines.tokens` specifies the special tokens to be colorized in the matched line.

### Available colors

The available colors are below.

- Black
- Blue
- Cyan
- Default
- Green
- LightBlack
- LightBlue
- LightCyan
- LightGreen
- LightMagenta
- LightRed
- LightWhite
- LightYellow
- Magenta
- Red
- White
- Yellow
