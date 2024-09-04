# Form UrlEncoded Plugin

This plugin support creating and matching comma-separated value text payloads.

## Building the plugin

The plugin is built with Rust. Just run `cargo build --release`. This will create the plugin binary file `pact-plugin-csv` 
in the `target/release/` directory.

## Installing the plugin

The plugin binary and [manifest file pact-plugin.json](pact-plugin.json) need to be copied into the `$HOME/.pact/plugins/csv-0.0.1` directory. You can download
those from the release for the plugin.

## Running with a development version of the plugin

If you build the plugin without the `--release`, this will create a debug version in the `target/debug` directory.
Copy the [manifest file pact-plugin.json](pact-plugin.json) into the `$HOME/.pact/plugins/csv-0.0.1` directory. If you
then edit that file, and set the `entryPoint` to the absolute path of the `pact-plugin-csv` binary in `target/debug`,
you can then make changes to the plugin, build it, and then all test projects will use that version.

## Example Projects

There are three example projects in [examples/csv](../../examples/csv) that use this plugin:

* csv-consumer-jvm - consumer written in Java
* csv-consumer-rust - consumer written in Rust
* csv-provider - provider written in Rust

## CSV matching definitions

The plugin matches the columns of the CSV data using matching rule definitions. The columns can be specified by
header (if the CSV has a header row) or by index (starting with 1).

Using the CSV from the example projects, it has 3 columns: Name, Number and Date. The matching rules can be specified by
(in pseudo config)

```javascript
"request.contents": {
  "pact:content-type": "application/x-www-form-urlencoded",                               // Set the content type to CSV
  "field:name": "matching(type,'Name')",                        // Field name must match by type
  "field:age", "matching(number,100)",                       // Field age must match a number format
  "field:dob", "matching(datetime, 'yyyy-MM-dd','2000-01-01')" // Field dob must match an ISO format yyyy-MM-dd
}
```
```

## Compatibility

<details><summary>Supported Platforms</summary>

| OS      | Architecture | Supported  | Pact CSV Plugin Version |
| ------- | ------------ | ---------  | ---------------- |
| OSX     | x86_64       | ✅         | All              |
| Linux   | x86_64       | ✅         | All              |
| Windows | x86_64       | ✅         | All              |
| OSX     | arm64        | ✅         | >=0.0.1          |
| Linux   | arm64        | ✅         | >=0.0.4          |
| Windows | arm64        | ✅         | >=0.0.6          |
| Alpine  | x86_64       | ✅         | >=0.0.6          |
| Alpine  | arm64        | ✅         | >=0.0.6          |

_Note:_ From v0.0.6, Linux executables are statically built with `musl` and as designed to work against `glibc` (eg, Debian) and `musl` (eg, Alpine) based distos.

</details>
