/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
/*! Module handling mouse events
*/
#![warn(missing_docs)]

use crate::graphics::Point;
use crate::item_tree::ItemVisitorResult;
use crate::items::{ItemRc, ItemRef, ItemWeak};
use crate::Property;
use crate::{component::ComponentRc, SharedString};
use const_field_offset::FieldOffsets;
use euclid::default::Vector2D;
use std::pin::Pin;
use std::rc::Rc;

/// The type of a MouseEvent
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MouseEventType {
    /// The mouse was pressed
    MousePressed,
    /// The mouse was relased
    MouseReleased,
    /// The mouse position has changed
    MouseMoved,
    /// The mouse exited the item or component
    MouseExit,
}

/// Structur representing a mouse event
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MouseEvent {
    /// The position of the cursor
    pub pos: Point,
    /// The action performed (pressed/released/moced)
    pub what: MouseEventType,
}

/// This value is returned by the input handler of a component
/// to notify the run-time about how the event was handled and
/// what the next steps are.
#[repr(C)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum InputEventResult {
    /// The event was accepted. This may result in additional events, for example
    /// accepting a mouse move will result in a MouseExit event later.
    EventAccepted,
    /// The event was ignored.
    EventIgnored,
    /* /// Same as grab, but continue forwarding the event to children.
    /// If a child grab the mouse, the grabber will be stored in the item itself.
    /// Only item that have grabbed storage can return this.
    /// The new_grabber is a reference to a usize to store thenext grabber
    TentativeGrab {
        new_grabber: &'a Cell<usize>,
    },
    /// While we have a TentaztiveGrab
    Forward {
        to: usize,
    },*/
    /// All further mouse event need to be sent to this item or component
    GrabMouse,
    /// One must send an MouseExit when the mouse leave this item
    ObserveHover,
}

impl Default for InputEventResult {
    fn default() -> Self {
        Self::EventIgnored
    }
}

/// InternalKeyCode is used to certain keys to unicode characters, since our
/// public key event only exposes a string. This enum captures this mapping.
#[derive(Debug, PartialEq, Clone)]
pub enum InternalKeyCode {
    /// Code corresponding to the left cursor key - encoded as 0xE ASCII (shift out)
    Left,
    /// Code corresponding to the right cursor key -- encoded as 0xF ASCII (shift in)
    Right,
    /// Code corresponding to the home key -- encoded as 0x2 ASCII (start of text)
    Home,
    /// Code corresponding to the end key -- encoded as 0x3 ASCII (end of text)
    End,
    /// Code corresponding to the backspace key -- encoded as 0x7 ASCII (backspace)
    Back,
    /// Code corresponding to the delete key -- encoded as 0x7F ASCII (delete)
    Delete,
    /// Code corresponding to the return key -- encoded as 0xA ASCII (newline)
    Return,
}

const LEFT_CODE: char = '\u{000E}'; // shift out
const RIGHT_CODE: char = '\u{000F}'; // shift in
const HOME_CODE: char = '\u{0002}'; // start of text
const END_CODE: char = '\u{0003}'; // end of text
const BACK_CODE: char = '\u{0007}'; // backspace \b
const DELETE_CODE: char = '\u{007F}'; // cancel
const RETURN_CODE: char = '\u{000A}'; // \n

impl InternalKeyCode {
    /// Encodes the internal key code as string
    pub fn encode_to_string(&self) -> SharedString {
        match self {
            InternalKeyCode::Left => LEFT_CODE,
            InternalKeyCode::Right => RIGHT_CODE,
            InternalKeyCode::Home => HOME_CODE,
            InternalKeyCode::End => END_CODE,
            InternalKeyCode::Back => BACK_CODE,
            InternalKeyCode::Delete => DELETE_CODE,
            InternalKeyCode::Return => RETURN_CODE,
        }
        .to_string()
        .into()
    }
    /// Tries to see if the provided string corresponds to a single special
    /// encoded key.
    pub fn try_decode_from_string(str: &SharedString) -> Option<Self> {
        let mut chars = str.chars();
        let ch = chars.next();
        if ch.is_some() && chars.next().is_none() {
            Some(match ch.unwrap() {
                LEFT_CODE => Self::Left,
                RIGHT_CODE => Self::Right,
                HOME_CODE => Self::Home,
                END_CODE => Self::End,
                BACK_CODE => Self::Back,
                DELETE_CODE => Self::Delete,
                RETURN_CODE => Self::Return,
                _ => return None,
            })
        } else {
            None
        }
    }
}

/// KeyboardModifier provides booleans to indicate possible modifier keys
/// on a keyboard, such as Shift, Control, etc.
///
/// On macOS, the command key is mapped to the meta modifier.
///
/// On Windows, the windows key is mapped to the meta modifier.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub struct KeyboardModifiers {
    /// Indicates the alt key on a keyboard.
    pub alt: bool,
    /// Indicates the control key on a keyboard.
    pub control: bool,
    /// Indicates the shift key on a keyboard.
    pub shift: bool,
    /// Indicates the logo key on macOS and the windows key on Windows.
    pub meta: bool,
}

#[derive(Debug, Clone, PartialEq, strum_macros::EnumString, strum_macros::Display)]
#[repr(C)]
/// This enum defines the different kinds of key events that can happen.
pub enum KeyEventType {
    /// A key on a keyboard was pressed.
    KeyPressed,
    /// A key on a keyboard was released.
    KeyReleased,
}

impl Default for KeyEventType {
    fn default() -> Self {
        Self::KeyPressed
    }
}

/// Represents a key event sent by the windowing system.
#[derive(Debug, Clone, PartialEq, Default)]
#[repr(C)]
pub struct KeyEvent {
    /// The unicode representation of the key pressed.
    pub text: SharedString,
    /// The keyboard modifiers active at the time of the key press event.
    pub modifiers: KeyboardModifiers,
    /// Indicates whether the key was pressed or released
    pub event_type: KeyEventType,
}

/// Represents how an item's key_event handler dealt with a key event.
/// An accepted event results in no further event propagation.
#[repr(C)]
pub enum KeyEventResult {
    /// The event was handled.
    EventAccepted,
    /// The event was not handled and should be sent to other items.
    EventIgnored,
}

/// This event is sent to a component and items when they receive or loose
/// the keyboard focus.
#[derive(Debug, Clone, PartialEq)]
#[repr(C)]
pub enum FocusEvent {
    /// This event is sent when an item receives the focus.
    FocusIn,
    /// This event is sent when an item looses the focus.
    FocusOut,
    /// This event is sent when the window receives the keyboard focus.
    WindowReceivedFocus,
    /// This event is sent when the window looses the keyboard focus.
    WindowLostFocus,
}

/// The state which a window should hold for the mouse input
#[derive(Default)]
pub struct MouseInputState {
    /// The stack of item which contain the mouse cursor (or grab)
    item_stack: Vec<ItemWeak>,
    /// true if the top item of the stack has the mouse grab
    grabbed: bool,
}

/// Process the `mouse_event` on the `component`, the `mouse_grabber_stack` is the prebious stack
/// of mouse grabber.
/// Returns a new mouse grabber stack.
pub fn process_mouse_input(
    component: ComponentRc,
    mouse_event: MouseEvent,
    window: &crate::window::ComponentWindow,
    mouse_input_state: MouseInputState,
) -> MouseInputState {
    'grab: loop {
        if !mouse_input_state.grabbed || mouse_input_state.item_stack.is_empty() {
            break 'grab;
        };
        let mut event = mouse_event.clone();
        for it in mouse_input_state.item_stack.iter() {
            let item = if let Some(item) = it.upgrade() { item } else { break 'grab };
            let g = item.borrow().as_ref().geometry();
            event.pos -= g.origin.to_vector();
        }
        let grabber = mouse_input_state.item_stack.last().unwrap().upgrade().unwrap();
        return match grabber.borrow().as_ref().input_event(event, window, &grabber) {
            InputEventResult::GrabMouse => mouse_input_state,
            _ => Default::default(),
        };
    }

    // Send the Exit event.
    // FIXME: we should send the exit event only if they no longer have the mouse
    let mut pos = mouse_event.pos;
    for it in mouse_input_state.item_stack.iter() {
        let item = if let Some(item) = it.upgrade() { item } else { break };
        let g = item.borrow().as_ref().geometry();
        pos -= g.origin.to_vector();
        item.borrow().as_ref().input_event(
            MouseEvent { pos, what: MouseEventType::MouseExit },
            window,
            &item,
        );
    }

    let mut result = MouseInputState::default();
    type State = (Vector2D<f32>, Vec<ItemWeak>);
    crate::item_tree::visit_items(
        &component,
        crate::item_tree::TraversalOrder::FrontToBack,
        |comp_rc: &ComponentRc,
         item: core::pin::Pin<ItemRef>,
         item_index: usize,
         (offset, mouse_grabber_stack): &State|
         -> ItemVisitorResult<State> {
            let item_rc = ItemRc::new(comp_rc.clone(), item_index);

            let geom = item.as_ref().geometry();
            let geom = geom.translate(*offset);

            if geom.contains(mouse_event.pos) {
                let mut event2 = mouse_event.clone();
                event2.pos -= geom.origin.to_vector();
                match item.as_ref().input_event(event2, window, &item_rc) {
                    InputEventResult::EventAccepted => {
                        result.item_stack = mouse_grabber_stack.clone();
                        result.item_stack.push(item_rc.downgrade());
                        result.grabbed = false;
                        return ItemVisitorResult::Abort;
                    }
                    InputEventResult::EventIgnored => (),
                    InputEventResult::GrabMouse => {
                        result.item_stack = mouse_grabber_stack.clone();
                        result.item_stack.push(item_rc.downgrade());
                        result.grabbed = true;
                        return ItemVisitorResult::Abort;
                    }
                    InputEventResult::ObserveHover => {
                        result.item_stack = mouse_grabber_stack.clone();
                        result.item_stack.push(item_rc.downgrade());
                        result.grabbed = false;
                    }
                };
            }

            let mut mouse_grabber_stack = mouse_grabber_stack.clone();
            mouse_grabber_stack.push(item_rc.downgrade());
            ItemVisitorResult::Continue((geom.origin.to_vector(), mouse_grabber_stack))
        },
        (Vector2D::new(0., 0.), Vec::new()),
    );
    result
}

/// The TextCursorBlinker takes care of providing a toggled boolean property
/// that can be used to animate a blinking cursor. It's typically stored in the
/// Window using a Weak and set_binding() can be used to set up a binding on a given
/// property that'll keep it up-to-date. That binding keeps a strong reference to the
/// blinker. If the underlying item that uses it goes away, the binding goes away and
/// so does the blinker.
#[derive(FieldOffsets)]
#[repr(C)]
#[pin]
pub(crate) struct TextCursorBlinker {
    cursor_visible: Property<bool>,
    cursor_blink_timer: crate::timers::Timer,
}

impl TextCursorBlinker {
    /// Creates a new instance, wrapped in a Pin<Rc<_>> because the boolean property
    /// the blinker properties uses the property system that requires pinning.
    pub fn new() -> Pin<Rc<Self>> {
        Rc::pin(Self {
            cursor_visible: Property::new(true),
            cursor_blink_timer: Default::default(),
        })
    }

    /// Sets a binding on the provided property that will ensure that the property value
    /// is true when the cursor should be shown and false if not.
    pub fn set_binding(instance: Pin<Rc<TextCursorBlinker>>, prop: &Property<bool>) {
        instance.as_ref().cursor_visible.set(true);
        // Re-start timer, in case.
        Self::start(&instance);
        prop.set_binding(move || {
            TextCursorBlinker::FIELD_OFFSETS.cursor_visible.apply_pin(instance.as_ref()).get()
        });
    }

    /// Starts the blinking cursor timer that will toggle the cursor and update all bindings that
    /// were installed on properties with set_binding call.
    pub fn start(self: &Pin<Rc<Self>>) {
        if self.cursor_blink_timer.running() {
            self.cursor_blink_timer.restart();
        } else {
            let toggle_cursor = {
                let weak_blinker = pin_weak::rc::PinWeak::downgrade(self.clone());
                move || {
                    if let Some(blinker) = weak_blinker.upgrade() {
                        let visible = TextCursorBlinker::FIELD_OFFSETS
                            .cursor_visible
                            .apply_pin(blinker.as_ref())
                            .get();
                        blinker.cursor_visible.set(!visible);
                    }
                }
            };
            self.cursor_blink_timer.start(
                crate::timers::TimerMode::Repeated,
                std::time::Duration::from_millis(500),
                toggle_cursor,
            );
        }
    }

    /// Stops the blinking cursor timer. This is usually used for example when the window that contains
    /// text editable elements looses the focus or is hidden.
    pub fn stop(&self) {
        self.cursor_blink_timer.stop()
    }
}
