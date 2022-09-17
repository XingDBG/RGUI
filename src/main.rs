#![windows_subsystem = "windows"]

use druid::widget::prelude::*;
use druid::widget::{Button, Flex, Label, TextBox, List, Scroll, MainAxisAlignment, CrossAxisAlignment, Switch};
use druid::im::{vector, Vector};
use druid::{AppLauncher, AppDelegate, DelegateCtx, Command, Target, Data, Handled, Lens, PlatformError, Widget, WidgetExt, WindowDesc, FileDialogOptions, UnitPoint, Color, ExtEventSink, Selector};
use std::ffi::OsString;
use std::error::Error;
use grep::regex::{RegexMatcherBuilder};
use grep::searcher::{BinaryDetection, SearcherBuilder, Encoding};
use walkdir::WalkDir;
use grep_searcher::sinks::UTF8;
use grep::matcher::Matcher;
use std::{thread};



#[derive(Clone, Default, Data, Lens)]
pub struct MatchResult {
    pub path: String,
    pub lnum: u64,
    pub line: String,
    pub start: usize,
    pub end: usize,
}


#[derive(Clone, Data, Lens)]
struct GUIState {
    path: String,
    regex: String,
    encoding: String,
    skip_bin: bool,
    case_insensitive: bool,
    multi_line: bool,
    result: Vector<MatchResult>,
    running: bool
}

struct Delegate;

const MATCH_RESULT: Selector<MatchResult> = Selector::new("one_match_result");
const MATCH_FINISH: Selector<bool> = Selector::new("match_all_finish");

fn main() -> Result<(), PlatformError> {
    let main_window = WindowDesc::new(ui_builder())
        .title("RGUI")
        .window_size((450.0, 450.0));
    AppLauncher::with_window(main_window)
        .delegate(Delegate)
        // .log_to_console()
        .launch(GUIState{
            path: String::new(),
            regex: String::new(),
            encoding: "UTF-8".into(),
            skip_bin: false,
            case_insensitive: true,
            multi_line: true,
            result: vector![],
            running: false
        })
}

fn ui_builder() -> impl Widget<GUIState> {
    
    let folder_lab = Label::new("Folder:")
        .fix_width(120.0);
    let folder_box = TextBox::new()
        .fix_width(280.0)
        .lens(GUIState::path)
        .disabled_if(|data:&GUIState, _| data.running);

    let dir_btn = Button::new("...")
        .on_click(|ctx, _, _| {
            ctx.submit_command(druid::commands::SHOW_OPEN_PANEL.with(FileDialogOptions::new().select_directories()))
        })
        .fix_width(30.0)
        .disabled_if(|data:&GUIState, _| data.running);


    let search_btn = Button::new("Search")
        .on_click(|ctx, data: &mut GUIState, _| {
            data.running = true;
            data.result.clear();
            wrapped_search(
                ctx.get_external_handle(),
                data.regex.clone(),
                data.encoding.clone(),
                data.skip_bin,
                data.case_insensitive,
                data.multi_line,
                OsString::from(data.path.clone())
            );
        })
        .disabled_if(|data:&GUIState, _| data.running);

    let regex_lab = Label::new("Regex:")
        .fix_width(120.0);

    let regex_box = TextBox::new()
        .fix_width(310.0)
        .lens(GUIState::regex)
        .disabled_if(|data:&GUIState, _| data.running);

    let encoding_lab = Label::new("Encoding:")
        .fix_width(120.0);
    let encoding_box = TextBox::new()
        .fix_width(280.0)
        .lens(GUIState::encoding)
        .disabled_if(|data:&GUIState, _| data.running);

    let case_lab = Label::new("Case insensitive:")
        .fix_width(120.0);
    let case_swi = Switch::new()
        .lens(GUIState::case_insensitive)
        .disabled_if(|data:&GUIState, _| data.running);
    
    let multi_lab = Label::new("Multi line:")
        .fix_width(120.0);
    let multi_swi = Switch::new()
        .lens(GUIState::multi_line)
        .disabled_if(|data:&GUIState, _| data.running);
    


    Flex::column()
        .main_axis_alignment(MainAxisAlignment::Start)
        .cross_axis_alignment(CrossAxisAlignment::Fill)
        .with_default_spacer()
        .with_child(Flex::row()
            .with_child(folder_lab)
            .with_child(folder_box)
            .with_child(dir_btn)
            .with_flex_spacer(1.0)
        )
        .with_default_spacer()
        .with_child(Flex::row()
            .with_child(regex_lab)
            .with_child(regex_box)
        )
        .with_default_spacer()
        .with_child(Flex::row()
            .with_child(encoding_lab)
            .with_child(encoding_box)
        )
        .with_default_spacer()
        .with_child(Flex::row()
            .with_child(case_lab)
            .with_child(case_swi)
        )
        .with_default_spacer()
        .with_child(Flex::row()
            .with_child(multi_lab)
            .with_child(multi_swi)
        )
        .with_default_spacer()
        .with_child(search_btn)
        .with_default_spacer()
        .with_flex_child(Scroll::new(List::new(|| {
                Label::new(|item: &MatchResult, _: &_| format!("{}:{}:{}-{}\t{}", item.path, item.lnum, item.start, item.end, item.line))
                    .align_vertical(UnitPoint::LEFT)
                    .padding(10.0)
                    .expand()
                    .height(50.0)
                    .background(Color::rgb(0.5, 0.5, 0.5))
            }))
            .vertical()
            .lens(GUIState::result),
         1.0)
}

impl AppDelegate<GUIState> for Delegate {
    fn command(&mut self, _ctx: &mut DelegateCtx, _target: Target, cmd: &Command, data: &mut GUIState, _env: &Env) -> Handled {
        if let Some(file_info) = cmd.get(druid::commands::OPEN_FILE) {
            data.path = file_info.path.to_str().expect("What the fuck?").to_string();
            return Handled::Yes;
        }
        if let Some(match_result) = cmd.get(MATCH_RESULT) {
            data.result.push_back(match_result.clone());
            return Handled::Yes;
        }
        if let Some(_) = cmd.get(MATCH_FINISH) {
            data.running = false;
            return Handled::Yes;
        }
        Handled::No
    }
}

fn wrapped_search(
    sink: ExtEventSink,
    pattern: String,
    encoding: String,
    skip_bin: bool,
    case_ins: bool,
    multi_l: bool,
    path: OsString
) {
    thread::spawn(move || {
        search(sink, pattern, encoding, skip_bin, case_ins, multi_l, path).unwrap();
    });
}

fn search(
    sink: ExtEventSink,
    pattern: String,
    encoding: String,
    skip_bin: bool,
    case_ins: bool,
    multi_l: bool,
    path: OsString
) -> Result<(), Box<dyn Error>> {
    // let matcher = RegexMatcher::new(&pattern)?;
    let matcher = RegexMatcherBuilder::new()
        .case_insensitive(case_ins)
        .multi_line(multi_l)
        .build(&pattern)?;
    let enc = Encoding::new(&encoding)?;
    let mut searcher = SearcherBuilder::new()
        .binary_detection(if skip_bin {BinaryDetection::none()} else {BinaryDetection::quit(b'\x00')})
        .line_number(true)
        .encoding(Some(enc))
        .build();

    for result in WalkDir::new(path) {
        let dent = match result {
            Ok(dent) => dent,
            Err(err) => {
                eprintln!("{}", err);
                continue;
            }
        };
        if !dent.file_type().is_file() {
            continue;
        }
        let result = searcher.search_path(
            &matcher,
            dent.path(),
            UTF8(|lnum, line| {
                    // We are guaranteed to find a match, so the unwrap is OK.
                    let mymatch = matcher.find(line.as_bytes())?.unwrap();
                    // println!("file:{}, line: {}, str: {}", dent.path().to_str().unwrap(), lnum, line);

                    sink.submit_command(MATCH_RESULT, MatchResult{
                        path: dent.path().to_str().expect("...").to_string(),
                        lnum, line: String::from(line), 
                        start: mymatch.start(), end: mymatch.end()
                    }, Target::Auto).expect("command failed to submit");
                    Ok(true)
                }
            )
        );
        if let Err(err) = result {
            eprintln!("{}: {}", dent.path().display(), err);
        }
    }
    
    sink.submit_command(MATCH_FINISH, true, Target::Auto).expect("command failed to submit");
    Ok(())
}