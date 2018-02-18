// std
use std::collections::{BTreeMap, VecDeque};

// Internal
use INTERNAL_ERROR_MSG;
use app::parser::Parser;
use app::settings::AppSettings as AS;
use args::{Arg, ArgMatcher};
use args::settings::ArgSettings;

pub struct Usage<'a, 'b, 'c, 'z>(&'z Parser<'a, 'b, 'c>)
where
    'a: 'b,
    'b: 'c,
    'c: 'z;

impl<'a, 'b, 'c, 'z> Usage<'a, 'b, 'c, 'z> {
    pub fn new(p: &'z Parser<'a, 'b, 'c>) -> Self { Usage(p) }

    // Creates a usage string for display. This happens just after all arguments were parsed, but
    // before any subcommands have been parsed (so as to give subcommands their own usage
    // recursively)
    pub fn create_usage_with_title(&self) -> String {
        debugln!("usage::create_usage_with_title;");
        let mut usage = String::with_capacity(75);
        usage.push_str("USAGE:\n    ");
        usage.push_str(&*self.create_usage_no_title());
        usage
    }

    pub fn create_help_usage_with_title(&self) -> String {
        debugln!("usage::create_usage_with_title;");
        let mut usage = String::with_capacity(75);
        usage.push_str("USAGE:\n    ");
        usage.push_str(&*self.create_help_usage(true));

        usage
    }

    // Creates a usage string (*without title*) if one was not provided by the user manually.
    pub fn create_usage_no_title(&self) -> String {
        debugln!("usage::create_usage_no_title;");
        if let Some(u) = self.0.app.usage_str {
            String::from(&*u)
        } else {
            self.create_smart_usage()
        }
    }

    // Creates a usage string for display in help messages (i.e. not for errors)
    pub fn create_help_usage(&self, incl_reqs: bool) -> String {
        let mut usage = String::with_capacity(75);
        let name = self.0
            .app
            .usage
            .as_ref()
            .unwrap_or_else(|| self.0.app.bin_name.as_ref().unwrap_or(&self.0.app.name));
        usage.push_str(&*name);
        let req_string = if incl_reqs {
            let mut reqs: Vec<&'a str> = self.0.required.iter().map(|&r| r).collect();
            reqs.sort();
            reqs.dedup();
            self.get_required_usage_from(None, None, false)
                .iter()
                .fold(String::new(), |a, s| a + &format!(" {}", s)[..])
        } else {
            String::new()
        };

        let flags = self.needs_flags_tag();
        if flags && !self.0.is_set(AS::UnifiedHelpMessage) {
            usage.push_str(" [FLAGS]");
        } else if flags {
            usage.push_str(" [OPTIONS]");
        }
        if !self.0.is_set(AS::UnifiedHelpMessage)
            && opts!(self.0.app)
                .any(|o| !o.is_set(ArgSettings::Required) && !o.is_set(ArgSettings::Hidden))
        {
            usage.push_str(" [OPTIONS]");
        }

        usage.push_str(&req_string[..]);

        let has_last = positionals!(self.0.app).any(|p| p.is_set(ArgSettings::Last));
        // places a '--' in the usage string if there are args and options
        // supporting multiple values
        if opts!(self.0.app).any(|o| o.is_set(ArgSettings::MultipleValues))
            && positionals!(self.0.app).any(|p| !p.is_set(ArgSettings::Required))
            && !(self.0.app.has_visible_subcommands()
                || self.0.is_set(AS::AllowExternalSubcommands)) && !has_last
        {
            usage.push_str(" [--]");
        }
        let not_req_or_hidden = |p: &Arg| {
            (!p.is_set(ArgSettings::Required) || p.is_set(ArgSettings::Last))
                && !p.is_set(ArgSettings::Hidden)
        };
        if positionals!(self.0.app).any(not_req_or_hidden) {
            if let Some(args_tag) = self.get_args_tag(incl_reqs) {
                usage.push_str(&*args_tag);
            } else {
                usage.push_str(" [ARGS]");
            }
            if has_last && incl_reqs {
                let pos = positionals!(self.0.app)
                    .find(|p| p.is_set(ArgSettings::Last))
                    .expect(INTERNAL_ERROR_MSG);
                debugln!("usage::create_help_usage: '{}' has .last(true)", pos.name);
                let req = pos.is_set(ArgSettings::Required);
                if req && positionals!(self.0.app).any(|p| !p.is_set(ArgSettings::Required)) {
                    usage.push_str(" -- <");
                } else if req {
                    usage.push_str(" [--] <");
                } else {
                    usage.push_str(" [-- <");
                }
                usage.push_str(&*pos.name_no_brackets());
                usage.push_str(">");
                usage.push_str(pos.multiple_str());
                if !req {
                    usage.push_str("]");
                }
            }
        }

        // incl_reqs is only false when this function is called recursively
        if self.0.app.has_visible_subcommands() && incl_reqs
            || self.0.is_set(AS::AllowExternalSubcommands)
        {
            if self.0.is_set(AS::SubcommandsNegateReqs) || self.0.is_set(AS::ArgsNegateSubcommands)
            {
                if !self.0.is_set(AS::ArgsNegateSubcommands) {
                    usage.push_str("\n    ");
                    usage.push_str(&*self.create_help_usage(false));
                    usage.push_str(" <SUBCOMMAND>");
                } else {
                    usage.push_str("\n    ");
                    usage.push_str(&*name);
                    usage.push_str(" <SUBCOMMAND>");
                }
            } else if self.0.is_set(AS::SubcommandRequired)
                || self.0.is_set(AS::SubcommandRequiredElseHelp)
            {
                usage.push_str(" <SUBCOMMAND>");
            } else {
                usage.push_str(" [SUBCOMMAND]");
            }
        }
        usage.shrink_to_fit();
        debugln!("usage::create_help_usage: usage={}", usage);
        usage
    }

    // Creates a context aware usage string, or "smart usage" from currently used
    // args, and requirements
    fn create_smart_usage(&self) -> String {
        debugln!("usage::smart_usage;");
        let reqs = self.get_required_usage_from(None, None, false);
        self.create_smart_usage_from_reqs(&*reqs)
    }

    // requirements have already been calculated
    pub(crate) fn create_smart_usage_from_reqs<I, T>(&self, reqs: I) -> String
    where
        T: AsRef<str>,
        I: IntoIterator<Item = T>,
    {
        let mut usage = String::with_capacity(75);
        let r_string = reqs.into_iter().fold(String::new(), |acc, s| {
            acc + &format!(" {}", s.as_ref())[..]
        });
        usage.push_str(
            &self.0
                .app
                .usage
                .as_ref()
                .unwrap_or_else(|| self.0.app.bin_name.as_ref().unwrap_or(&self.0.app.name))[..],
        );
        usage.push_str(&*r_string);
        if self.0.is_set(AS::SubcommandRequired) {
            usage.push_str(" <SUBCOMMAND>");
        }
        usage.shrink_to_fit();
        usage
    }

    // Gets the `[ARGS]` tag for the usage string
    fn get_args_tag(&self, incl_reqs: bool) -> Option<String> {
        debugln!("usage::get_args_tag;");
        let mut count = 0;
        'outer: for pos in positionals!(self.0.app)
            .filter(|pos| !pos.is_set(ArgSettings::Required))
            .filter(|pos| !pos.is_set(ArgSettings::Hidden))
            .filter(|pos| !pos.is_set(ArgSettings::Last))
        {
            debugln!("usage::get_args_tag:iter:{}:", pos.name);
            if let Some(g_vec) = self.0.groups_for_arg(pos.name) {
                for grp_s in &g_vec {
                    debugln!("usage::get_args_tag:iter:{}:iter:{};", pos.name, grp_s);
                    // if it's part of a required group we don't want to count it
                    if groups!(self.0.app).any(|g| g.required && (&g.name == grp_s)) {
                        continue 'outer;
                    }
                }
            }
            count += 1;
            debugln!(
                "usage::get_args_tag:iter: {} Args not required or hidden",
                count
            );
        }
        if !self.0.is_set(AS::DontCollapseArgsInUsage) && count > 1 {
            debugln!("usage::get_args_tag:iter: More than one, returning [ARGS]");
            return None; // [ARGS]
        } else if count == 1 && incl_reqs {
            let pos = positionals!(self.0.app)
                .find(|pos| {
                    !pos.is_set(ArgSettings::Required) && !pos.is_set(ArgSettings::Hidden)
                        && !pos.is_set(ArgSettings::Last)
                })
                .expect(INTERNAL_ERROR_MSG);
            debugln!(
                "usage::get_args_tag:iter: Exactly one, returning '{}'",
                pos.name
            );
            return Some(format!(
                " [{}]{}",
                pos.name_no_brackets(),
                pos.multiple_str()
            ));
        } else if self.0.is_set(AS::DontCollapseArgsInUsage) && self.0.has_positionals()
            && incl_reqs
        {
            debugln!("usage::get_args_tag:iter: Don't collapse returning all");
            return Some(
                positionals!(self.0.app)
                    .filter(|pos| !pos.is_set(ArgSettings::Required))
                    .filter(|pos| !pos.is_set(ArgSettings::Hidden))
                    .filter(|pos| !pos.is_set(ArgSettings::Last))
                    .map(|pos| format!(" [{}]{}", pos.name_no_brackets(), pos.multiple_str()))
                    .collect::<Vec<_>>()
                    .join(""),
            );
        } else if !incl_reqs {
            debugln!("usage::get_args_tag:iter: incl_reqs=false, building secondary usage string");
            let highest_req_pos = positionals!(self.0.app)
                .filter_map(|pos| {
                    if pos.is_set(ArgSettings::Required) && !pos.is_set(ArgSettings::Last) {
                        Some(pos.index)
                    } else {
                        None
                    }
                })
                .max()
                .unwrap_or_else(|| Some(positionals!(self.0.app).count() as u64));
            return Some(
                positionals!(self.0.app)
                    .filter_map(|pos| {
                        if pos.index <= highest_req_pos {
                            Some(pos)
                        } else {
                            None
                        }
                    })
                    .filter(|pos| !pos.is_set(ArgSettings::Required))
                    .filter(|pos| !pos.is_set(ArgSettings::Hidden))
                    .filter(|pos| !pos.is_set(ArgSettings::Last))
                    .map(|pos| format!(" [{}]{}", pos.name_no_brackets(), pos.multiple_str()))
                    .collect::<Vec<_>>()
                    .join(""),
            );
        }
        Some("".into())
    }

    // Determines if we need the `[FLAGS]` tag in the usage string
    fn needs_flags_tag(&self) -> bool {
        debugln!("usage::needs_flags_tag;");
        'outer: for f in flags!(self.0.app) {
            debugln!("usage::needs_flags_tag:iter: f={};", f.name);
            if let Some(l) = f.long {
                if l == "help" || l == "version" {
                    // Don't print `[FLAGS]` just for help or version
                    continue;
                }
            }
            if let Some(g_vec) = self.0.groups_for_arg(f.name) {
                for grp_s in &g_vec {
                    debugln!("usage::needs_flags_tag:iter:iter: grp_s={};", grp_s);
                    if groups!(self.0.app).any(|g| &g.name == grp_s && g.required) {
                        debugln!("usage::needs_flags_tag:iter:iter: Group is required");
                        continue 'outer;
                    }
                }
            }
            if f.is_set(ArgSettings::Hidden) {
                continue;
            }
            debugln!("usage::needs_flags_tag:iter: [FLAGS] required");
            return true;
        }

        debugln!("usage::needs_flags_tag: [FLAGS] not required");
        false
    }

    // Returns the required args in usage string form by fully unrolling all groups
    pub fn get_required_usage_from(
        &self,
        matcher: Option<&ArgMatcher<'a>>,
        extra: Option<&'a str>,
        incl_last: bool,
    ) -> Vec<String> {
        debugln!(
            "usage::get_required_usage_from: reqs={:?}, extra={:?}",
            reqs,
            extra
        );
        let unrolled = self.0.get_required_unrolled(extra);
        let mut ret_val = VecDeque::new();
        let args_in_groups = groups!(self.0.app)
            .filter(|gn| unrolled.contains(&gn.name))
            .flat_map(|g| self.0.arg_names_in_group(g.name))
            .collect::<Vec<_>>();

        let pmap = if let Some(m) = matcher {
            unrolled
                .iter()
                .filter(|a| self.0.positionals.values().any(|p| &p == a))
                .filter(|&pos| !m.contains(pos))
                .filter_map(|pos| find!(self.0.app, pos))
                .filter(|&pos| incl_last || !pos.is_set(ArgSettings::Last))
                .filter(|pos| !args_in_groups.contains(&pos.name))
                .map(|pos| (pos.index.unwrap(), pos))
                .collect::<BTreeMap<u64, &Arg>>() // sort by index
        } else {
            unrolled
                .iter()
                .filter(|a| self.0.positionals.values().any(|p| &p == a))
                .filter_map(|pos| find!(self.0.app, pos))
                .filter(|&pos| incl_last || !pos.is_set(ArgSettings::Last))
                .filter(|pos| !args_in_groups.contains(&pos.name))
                .map(|pos| (pos.index.unwrap(), pos))
                .collect::<BTreeMap<u64, &Arg>>() // sort by index
        };
        debugln!(
            "usage::get_required_usage_from: args_in_groups={:?}",
            args_in_groups
        );
        debugln!("usage::get_required_usage_from: pmap={:?}", pmap);
        for &p in pmap.values() {
            let s = p.to_string();
            if args_in_groups.is_empty() || !args_in_groups.contains(&&*s) {
                ret_val.push_back(s);
            }
        }
        for a in unrolled
            .iter()
            .filter(|name| !positionals!(self.0.app).any(|p| &&p.name == name))
            // .filter(|name| !groups!(self.0.app).any(|g| &&g.name == name))
            .filter(|name| !args_in_groups.contains(&(name.as_ref())))
        // .filter(|name| !(matcher.is_some() && matcher.as_ref().unwrap().contains(name)))
        {
            debugln!("usage::get_required_usage_from:iter:{}:", a);
            let arg = find!(self.0.app, a)
                .map(|f| f.to_string())
                .expect(INTERNAL_ERROR_MSG);
            ret_val.push_back(arg);
        }
        let mut g_vec: Vec<String> = vec![];
        for g in unrolled
            .iter()
            .filter(|n| groups!(self.0.app).any(|g| &&g.name == n))
        {
            let g_string = self.0.args_in_group(g).join("|");
            let elem = format!("<{}>", &g_string[..g_string.len()]);
            if !g_vec.contains(&elem) {
                g_vec.push(elem);
            }
        }
        for g in g_vec {
            ret_val.push_back(g);
        }

        ret_val.into()
    }
}
