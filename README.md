# wtd
The goal is to manage tasks, events, etc. in plaintext (vimwiki/markdown), then
spit out a publicly-viewable version.

First, create a `wtd.md` file of the following format:
```
# 12/27/21
## Monday
- [X] Some task... @10AM+1h +self
- [ ] Another task... @3PM--4:45PM +busy

## Tuesday
- [ ] Etc. @9:30AM+30m +busy +public +join-me

# 1/2/22
## Monday
- [ ] Group meeting @12PM+1h +busy
```
Top-level headings should be used to indicate weeks, second-level headings
days.

Tasks/events start with either `- [ ]` or `- [X]`. Times of the form `@S--E` or
`@S+D` as well as tags of the form `+tag` are pulled out of the task
description automatically.

By default, event descriptions are private. Adding the `public` tag prints the
event description on the calendar page. Other tags are ignored by default,
unless they are mentioned in the `public_tags` hashmap in `src/main.rs`, in
which case they are printed out to the public calendar along with a short
description. Events on the public calendar can also be styled according to
these `public_tags`, see `calendar_style.css`.

To generate the public calendar, run:
```
$ cargo run > calendar.html
```

The calendar does not require Javascript and should work very well in, e.g.,
`w3m`.

#### Rust Warning
This is my first time writing a project in Rust; it's very likely there are
major issues. Proceed with caution. Feedback appreciated.

#### Acks
I think the first time I came across the idea of sharing a calendar publicly on
the Web was via [Prof. Walker's website](https://www.cs.princeton.edu/~dpw/).

I've seen the notion of a large text file with tags to pull out some amount of
structure done well in [ledger](https://www.ledger-cli.org/) and
[ideaflow](https://www.ideaflow.io/).
