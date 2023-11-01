# course2zwift

A simple app to create Zwift workouts from CSV files.

It allows you to create Zwift workouts based on table data
with time, power and (optional) hint text snippets.
You can scale it by time and power, to adapt to your own abilities.

## Build

It's a rust app. Use `cargo build` to build it.

## Usage

```bash
# When ready, pipe the output to a workout file by adding
# > ~/Documents/Zwift/Workouts/<your-id>/my_course.zwo
# to the command.
$ ./course2zwift [OPTIONS] <course-name> <your-ftp> <path-to-csv-file>
```

### Options

```bash
  -d, --description <DESCRIPTION>    optional description
  -A, --author <AUTHOR>              customizable author name
  -T, --sport-type <SPORT_TYPE>      customizable sport type [default: "ride"]
  -a, --acceleration <ACCELERATION>  time shrink factor [default: 1.0]
  -s, --scale <SCALE>                power scale factor [default: 1.0]
  -r, --raster <RASTER>              duration rasterization in seconds [default: 30]
  -h, --help                         Print help
```

## Data Provisioning

You can provide a CSV file like this table one:

| time     | power | text         |
|----------|-------|--------------|
| 00:00:00 | 180   |              |
| 00:01:30 |       | Turn right   |
| 00:02:00 | 210   | Up that hill |
| 00:03:30 | 160   |              |
| 00:04:30 |       | You're done! |

The corresponding raw CSV file then looks as follows:
```text
"00:00:00",180,
"00:01:30",,"Turn right"
"00:02:00",210,"Up that hill"
"00:03:30",160,
"00:04:30",,"You're done!"
```

## Hints

Be careful to use a rasterization size to match the granularity of your file,
especially if you want to ride along with a pre-recorded video:
- If the raster is too small, it will create potentially very short segments,
which means you need to change gear quite rapidly in those situations.
- If the raster is too big, it won't keep up with the course well.
  Every segment is at least as long as the raster size.
