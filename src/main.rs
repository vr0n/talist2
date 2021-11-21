use rusqlite::{
  Connection,
  params
};
use chrono::prelude::*;
use crossterm::{
  event::{
    self,
    Event as CEvent,
    KeyCode
  },
  terminal::{
    disable_raw_mode,
    enable_raw_mode
  },
};
//use serde::{
//  Deserialize,
//  Serialize
//};
use std::io;
use std::sync::mpsc;
use std::thread;
use std::time::{
  Duration,
  Instant
};
//use thiserror::Error;
use tui::{
  backend::CrosstermBackend,
  layout::{
    Constraint,
    Direction,
    Layout
  },
  style::{
    Color,
    Modifier,
    Style
  },
  text::{
    Span,
    Spans
  },
  widgets::{
    Block,
    BorderType,
    Borders,
    Cell,
    List,
    ListItem,
    ListState,
    Row,
    Table,
    Tabs,
  },
  Terminal,
};

/******************
  END DEPENDENCIES
*******************/

const DB_PATH: &str = "./data/talist.db";

enum Event<I> {
  Input(I),
  Tick,
}

#[derive(Clone, Debug)]
struct Ticket {
  task: String,
  description: String,
  category: String,
  priority: String,
  board: String,
  created_date: String, // TODO: Change created, due, finished, and duration to DateTimes once I figure out the FromSql error
  due_date: String,
  finished_date: String,
  duration: String,
}

#[derive(Copy, Clone, Debug)]
enum MenuItem {
  Tickets,
}

impl From<MenuItem> for usize {
    fn from(input: MenuItem) -> usize {
        match input {
            MenuItem::Tickets => 0,
        }
    }
}

/* 
  Get Items Function
*/
fn get_items(list_board: &str) -> Result<Vec<Ticket>, rusqlite::Error> {
  let conn = Connection::open(DB_PATH)?;
  let mut stmt = conn.
    prepare(
      "SELECT
        task,
        description,
        category,
        priority,
        board,
        created_date,
        due_date,
        finished_date,
        duration
      FROM
        items
      WHERE
        board = :board"
    )?; // Grab the struct fields explicitly so we can edit the DB without breaking the code

  let rows = stmt
    .query_map(&[(":board", list_board)], |row| {
      Ok(Ticket {
        task: row.get(0)?,
        description: row.get(1)?,
        category: row.get(2)?,
        priority: row.get(3)?,
        board: row.get(4)?,
        created_date: row.get(5)?,
        due_date: row.get(6)?,
        finished_date: row.get(7)?,
        duration: row.get(8)?,
      })
    })?;

  let mut parsed = Vec::new();
  for row in rows {
    parsed.push(row?);
  }

  if parsed.len() == 0 {
    let blank_ticket =
      Ticket {
        task: "".to_string(),
        description: "".to_string(),
        category: "".to_string(),
        priority: "".to_string(),
        board: list_board.to_string(),
        created_date: "".to_string(),
        due_date: "".to_string(),
        finished_date: "".to_string(),
        duration: "".to_string()
      };

    parsed.push(blank_ticket);
  }

  // TODO: This code must return at least one ticket, or there is a panic thrown by the "expects" call after this function is called
  // This means we have a couple options:
  // 1. We work around the "expects" call. This may be possible, but when I just removed it, I saw a bunch of errors, so I'm leaving it alone for now
  // 2. We put a check here for "if rows.len() == 0" and we return an empty Ticket struct (probably what we will do)
  // We are going with 2 for now, but 1 seems more correct...
  Ok(parsed)
}

/*
  Get Boards Function
*/
fn get_boards() -> Result<Vec<String>, rusqlite::Error> {
  let conn = Connection::open(DB_PATH)?;
  let mut stmt = conn.
    prepare(
      "SELECT
        name
      FROM
        lists"
    )?;

  let rows = stmt
    .query_map([], |row| {
      Ok(row.get(0)?)
    })?;

  let mut boards: Vec<String> = Vec::new();
  for row in rows {
    boards.push(row?);
  }

  Ok(boards)
}

/*
  Main Function
*/
fn main() -> Result<(), Box<dyn std::error::Error>> {
    enable_raw_mode().expect("can run in raw mode");

    let mut boards = &get_boards()?;
    let mut board = &get_boards()?[0];
    let (tx, rx) = mpsc::channel();
    let tick_rate = Duration::from_millis(200);
    thread::spawn(move || {
        let mut last_tick = Instant::now();
        loop {
            let timeout = tick_rate
                .checked_sub(last_tick.elapsed())
                .unwrap_or_else(|| Duration::from_secs(0));

            if event::poll(timeout).expect("poll works") {
                if let CEvent::Key(key) = event::read().expect("can read events") {
                    tx.send(Event::Input(key)).expect("can send events");
                }
            }

            if last_tick.elapsed() >= tick_rate {
                if let Ok(_) = tx.send(Event::Tick) {
                    last_tick = Instant::now();
                }
            }
        }
    });

    let stdout = io::stdout();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let menu_titles = vec!["h/l- Switch Board", "a- Add Ticket", "d- Delete Ticket", "q- Quit", "?- Full Help Menu"];
    let mut active_menu_item = MenuItem::Tickets;
    let mut ticket_list_state = ListState::default(); // Pointer to ListState
    ticket_list_state.select(Some(0));

    loop {
        terminal.draw(|rect| {
          let size = rect.size(); // Size of terminal
          let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(0) // Get as close to the size of the terminal as we can while looking good.
            .constraints(
                [
                    Constraint::Length(3), // Minimum height of elements
                    Constraint::Min(2), // It's not obvious what this is, but it certainly can't be 1.
                ]
                .as_ref(),
            )
            .split(size); // Set chunks to size of terminal

          let menu = menu_titles // "Menu" is just for our mini-help menu
              .iter()
              .map(|t| {
                  let menu_item: Vec<&str> = t.split('-').collect(); // TODO: We get to choose our own format, since this menu is static, but maybe we should change this to just use whitspace instead of '-'
                  let func_key = menu_item[0]; // We split each menu item into keystroke and what it does
                  let func_desc = menu_item[1];
                  Spans::from(vec![
                      Span::styled(
                          func_key,
                          Style::default()
                              .fg(Color::Yellow),
                      ),
                      Span::styled(func_desc, Style::default().fg(Color::White)),
                  ])
              })
              .collect();

          let tabs = Tabs::new(menu)
              .block(Block::default().title("Help").borders(Borders::ALL));

          rect.render_widget(tabs, chunks[0]); // Render help menu
          match active_menu_item {
              MenuItem::Tickets => {
                  let tickets_chunks = Layout::default()
                      .direction(Direction::Horizontal)
                      .constraints(
                          [Constraint::Percentage(25), Constraint::Percentage(80)].as_ref(), // First constraint is ticket name
                      )
                      .split(chunks[1]);
                  let (left, center, right) = render_tickets(&ticket_list_state, board.to_string());
                  rect.render_stateful_widget(left, tickets_chunks[0], &mut ticket_list_state);
                  rect.render_widget(right, tickets_chunks[1]);
              }
          }
        })?;

        match rx.recv()? {
          Event::Input(event) => match event.code {
              KeyCode::Char('q') => {
                disable_raw_mode()?;
                terminal.show_cursor()?;
                break;
              }
              KeyCode::Char('t') => active_menu_item = MenuItem::Tickets,
              KeyCode::Char('a') => {
                add_ticket(board);
              }
              KeyCode::Char('l') => {
                let old_index = boards.iter().position(|brd| brd == board).unwrap();
                let index = (old_index + 1) % boards.len();
                board = &boards[index];
              }
              KeyCode::Char('h') => {
                let old_index = boards.iter().position(|brd| brd == board).unwrap();
                let mut index = 0;

                if old_index == 0 {
                  index = boards.len() - 1;
                } else {
                  index = (old_index - 1) % boards.len();
                }
                board = &boards[index];
              }
              KeyCode::Char('j') => {
                  if let Some(selected) = ticket_list_state.selected() {
                      let amount_tickets = get_items(&board).expect("can fetch ticket list").len();
                      if selected >= amount_tickets - 1 {
                          ticket_list_state.select(Some(0));
                      } else {
                          ticket_list_state.select(Some(selected + 1));
                      }
                  }
              }
              KeyCode::Char('k') => {
                  if let Some(selected) = ticket_list_state.selected() {
                      let amount_tickets = get_items(&board).expect("can fetch ticket list").len();
                      if selected == 0 {
                          ticket_list_state.select(Some(amount_tickets - 1));
                      } else {
                          ticket_list_state.select(Some(selected - 1));
                      }
                  }
              }
              _ => {}
          },
          Event::Tick => {}
        }
    }

    Ok(())
}

/*
  Add Ticket Function
*/
fn add_ticket(board: &str) -> Result<(), rusqlite::Error> {
  let mut ticket = Ticket {
    task: "".to_string(),
    description: "".to_string(),
    category: "".to_string(),
    priority: "".to_string(),
    board: board.to_string(),
    created_date: Utc::now().to_string(),
    due_date: "".to_string(),
    finished_date: "".to_string(),
    duration: "".to_string()
  };

  let mut input: String = ("").to_string();
  
  loop {
    if let event::Event::Key(key) = event::read().expect("can read events") {
      match key.code {
        KeyCode::Enter => { break }
        KeyCode::Backspace => {
          if input.len() != 0 {
            input.pop();
          }
        }
        KeyCode::Char(c) => {
          input.push(c);
        }
        _ => {}
      }
    }
  }

  ticket.task = input;

  let conn = Connection::open(DB_PATH)?;
  conn.execute(
    "INSERT
    INTO
      items
    (
      task,
      description,
      category,
      priority,
      board,
      created_date,
      due_date,
      finished_date,
      duration
    )
    VALUES
    (
      ?1,
      ?2,
      ?3,
      ?4,
      ?5,
      ?6,
      ?7,
      ?8,
      ?9
    )",
    params![
      ticket.task,
      ticket.description,
      ticket.category,
      ticket.priority,
      ticket.board,
      ticket.created_date,
      ticket.due_date,
      ticket.finished_date,
      ticket.duration
    ],
  )?;

  Ok(())
}

/*
  Get Current Ticket Function
*/
fn get_current_ticket(tickets_list_state: &ListState, ticket_list: Vec<Ticket>) -> Ticket {
    let selected_ticket = ticket_list
        .get(
            tickets_list_state
                .selected()
                .expect("there is always a selected ticket"),
        )
        .expect("exists")
        .clone();

    return selected_ticket;
}

/*
  Render Tickets Function
*/
fn render_tickets<'a>(tickets_list_state: &ListState, board: String) -> (List<'a>, Table<'a>) {
    let ticket_list = get_items(&board).expect("can fetch ticket list");

    let items: Vec<_> = ticket_list
        .iter()
        .map(|ticket| {
            ListItem::new(Spans::from(vec![Span::styled(
                ticket.task.clone(),
                Style::default(),
            )]))
        })
        .collect();

    let selected_ticket = get_current_ticket(tickets_list_state, ticket_list);

    let tickets = Block::default()
        .borders(Borders::ALL)
        .style(Style::default().fg(Color::White))
        .title(Span::styled(selected_ticket.board, Style::default().fg(Color::Yellow)))
        .border_type(BorderType::Plain);

    let list = List::new(items).block(tickets).highlight_style( // Set highlight style for selected list item
        Style::default()
            .bg(Color::Yellow)
            .fg(Color::Black),
    );

    let ticket_detail = Table::new( // Set details and break into rows
      vec![
        Row::new(vec![
          Cell::from(Span::raw(selected_ticket.task)),
          Cell::from(Span::raw(selected_ticket.description)),
          Cell::from(Span::raw(selected_ticket.category)),
          Cell::from(Span::raw(selected_ticket.duration.to_string())),
          Cell::from(Span::raw(selected_ticket.created_date.to_string())),
          Cell::from(Span::raw(selected_ticket.finished_date.to_string())),
        ]),
      ]
    )
    .header(
      Row::new(vec![
        Cell::from(Span::styled(
            "Task",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Cell::from(Span::styled(
            "Desc",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Cell::from(Span::styled(
            "Category",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Cell::from(Span::styled(
            "Duration",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Cell::from(Span::styled(
            "Created At",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Cell::from(Span::styled(
            "Finished At",
            Style::default().add_modifier(Modifier::BOLD),
        )),
      ]).bottom_margin(1), // Set space between header and value
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .style(Style::default().fg(Color::White))
            .title("Detail")
            .border_type(BorderType::Plain),
    )
    .widths(&[
        Constraint::Percentage(20),
        Constraint::Percentage(20),
        Constraint::Percentage(20),
        Constraint::Percentage(13),
        Constraint::Percentage(13),
        Constraint::Percentage(13),
    ])
    .column_spacing(2);

    (list, ticket_task, ticket_detail)
}

//fn add_random_ticket_to_db() -> Result<Vec<Ticket>, Error> {
//    let mut rng = rand::thread_rng();
//    let db_content = fs::read_to_string(DB_PATH)?;
//    let mut parsed: Vec<Ticket> = serde_json::from_str(&db_content)?;
//    let catsdogs = match rng.gen_range(0, 1) {
//        0 => "cats",
//        _ => "dogs",
//    };
//
//    let random_ticket = Ticket {
//        id: rng.gen_range(0, 9999999),
//        task: rng.sample_iter(Alphanumeric).take(10).collect(),
//        board: rng.sample_iter(Alphanumeric).take(10).collect(),
//        priority: rng.sample_iter(Alphanumeric).take(10).collect(),
//        description: rng.sample_iter(Alphanumeric).take(10).collect(),
//        category: catsdogs.to_owned(),
//        created_at: Utc::now(),
//        duration: Utc::now(),
//        finished_at: Utc::now(),
//    };
//
//    parsed.push(random_ticket);
//    fs::write(DB_PATH, &serde_json::to_vec(&parsed)?)?;
//    Ok(parsed)
//}

//fn remove_ticket_at_index(ticket_list_state: &mut ListState) -> Result<(), Error> {
//    if let Some(selected) = ticket_list_state.selected() {
//        let db_content = fs::read_to_string(DB_PATH)?;
//        let mut parsed: Vec<Ticket> = serde_json::from_str(&db_content)?;
//        parsed.remove(selected);
//        fs::write(DB_PATH, &serde_json::to_vec(&parsed)?)?;
//        ticket_list_state.select(Some(selected - 1));
//    }
//    Ok(())
//}
