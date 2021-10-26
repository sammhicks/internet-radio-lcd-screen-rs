use std::{fmt::Display, marker::PhantomData};

use crate::display::{Segment, TextDisplay};

pub enum WidgetEvent {
    Tick(std::time::Instant),
}

pub trait Widget: Sized {
    type Data;

    fn event(&mut self, event: &WidgetEvent, data: &Self::Data);
    fn update(&mut self, old_data: &Self::Data, data: &Self::Data);

    // Force a repaint the next time paint is called. Widgets should also clear any cached data
    fn force_repaint(&mut self, data: &Self::Data);

    fn paint(&mut self, data: &Self::Data, display: &mut impl TextDisplay);
}

pub struct FixedLabel<Data, S: AsRef<str>> {
    string: S,
    segment: Segment,
    should_paint: bool,
    _data: PhantomData<fn(&Data)>,
}

impl<Data, S: AsRef<str>> FixedLabel<Data, S> {
    pub fn new(string: S, segment: impl Into<Segment>) -> Self {
        Self {
            string,
            segment: segment.into(),
            should_paint: true,
            _data: PhantomData,
        }
    }
}

impl<Data, S: AsRef<str>> Widget for FixedLabel<Data, S> {
    type Data = Data;

    fn event(&mut self, _event: &WidgetEvent, _data: &Self::Data) {}

    fn update(&mut self, _old_data: &Self::Data, _data: &Self::Data) {}

    fn force_repaint(&mut self, _data: &Self::Data) {
        self.should_paint = true;
    }

    fn paint(&mut self, _data: &Self::Data, display: &mut impl TextDisplay) {
        if self.should_paint {
            self.should_paint = false;

            display.write_to(self.segment, self.string.as_ref());
        }
    }
}

pub struct GeneratedLabel<T: Display + PartialEq, G: FnMut() -> T, Data> {
    segment: Segment,
    generator: G,
    previous_value: Option<T>,
    _data: PhantomData<fn(&Data)>,
}

impl<T: Display + PartialEq, G: FnMut() -> T, Data> GeneratedLabel<T, G, Data> {
    pub fn new(segment: impl Into<Segment>, generator: G) -> Self {
        Self {
            segment: segment.into(),
            generator,
            previous_value: None,
            _data: PhantomData,
        }
    }
}

impl<T: Display + PartialEq, G: FnMut() -> T, Data> Widget for GeneratedLabel<T, G, Data> {
    type Data = Data;

    fn event(&mut self, _event: &WidgetEvent, _data: &Self::Data) {}

    fn update(&mut self, _old_data: &Self::Data, _data: &Self::Data) {}

    fn force_repaint(&mut self, _data: &Self::Data) {
        self.previous_value = None;
    }

    fn paint(&mut self, _data: &Self::Data, display: &mut impl TextDisplay) {
        let new_value = (self.generator)();

        if self.previous_value.as_ref() != Some(&new_value) {
            display.write_to(self.segment, &new_value);

            self.previous_value = Some(new_value);
        }
    }
}

enum TextAlignment {
    Left,
    Right,
}

pub struct Label<T: Display + PartialEq> {
    needs_repainting: bool,
    segment: Segment,
    text_alignment: TextAlignment,
    _data: PhantomData<fn(&T)>,
}

impl<T: Display + PartialEq> Label<T> {
    pub fn new(segment: impl Into<Segment>) -> Self {
        Self {
            needs_repainting: true,
            segment: segment.into(),
            text_alignment: TextAlignment::Left,
            _data: PhantomData,
        }
    }

    /// Right align the data. The data must respect text alignment when formatted
    pub fn align_right(mut self) -> Self {
        self.text_alignment = TextAlignment::Right;
        self
    }
}

impl<T: Display + PartialEq> Widget for Label<T> {
    type Data = T;

    fn event(&mut self, _event: &WidgetEvent, _data: &Self::Data) {}

    fn update(&mut self, old_data: &Self::Data, data: &Self::Data) {
        if old_data != data {
            self.needs_repainting = true;
        }
    }

    fn force_repaint(&mut self, _data: &Self::Data) {
        self.needs_repainting = true;
    }

    fn paint(&mut self, data: &Self::Data, display: &mut impl TextDisplay) {
        if self.needs_repainting {
            self.needs_repainting = false;

            match self.text_alignment {
                TextAlignment::Left => display.write_to(self.segment, data),
                TextAlignment::Right => display.write_to(
                    self.segment,
                    format_args!(
                        "{:>width$.width$}",
                        data,
                        width = self.segment.length as usize
                    ),
                ),
            }
        }
    }
}

pub struct ScrollingLabel<T: Display + PartialEq> {
    needs_repainting: bool,
    start_position: usize,
    wait_ticks_remaining: usize,
    segment: Segment,
    text: Option<String>,
    _data: PhantomData<fn(&T)>,
}

impl<T: Display + PartialEq> ScrollingLabel<T> {
    const WAIT_BEFORE_SCROLLING_TICKS_COUNT: usize = 2; // The number of tics before scrolling begins
    const MAX_SCROLL: usize = 6; // The furthest distance (in characters) that a label will scroll
    const CHARACTERS_REMAINING_RESET_COUNT: usize = 6; // The number of remaining characters when scrolling restarts from the beginning

    pub fn new(segment: impl Into<Segment>) -> Self {
        Self {
            needs_repainting: true,
            start_position: 0,
            wait_ticks_remaining: 0,
            segment: segment.into(),
            text: None,
            _data: PhantomData,
        }
    }

    fn generate_text<'t>(text: &'t mut Option<String>, data: &T) -> &'t str {
        text.get_or_insert_with(|| data.to_string()).as_str()
    }

    fn reset_scroll(&mut self) {
        self.needs_repainting = true;
        self.start_position = 0;
        self.wait_ticks_remaining = Self::WAIT_BEFORE_SCROLLING_TICKS_COUNT;
    }

    fn update_scroll(&mut self, data: &T) {
        let text = Self::generate_text(&mut self.text, data);

        if text.chars().count() <= self.segment.length.into() {
            return;
        }

        if self.wait_ticks_remaining > 0 {
            self.wait_ticks_remaining -= 1;
            return;
        }

        self.needs_repainting = true;

        let visible_text = &text[self.start_position..];

        if visible_text.chars().count() <= Self::CHARACTERS_REMAINING_RESET_COUNT {
            self.reset_scroll();
            return;
        }

        if let Some((_n, (i, _c))) = visible_text
            .char_indices()
            .enumerate()
            .skip_while(|&(n, (_i, c))| (n < (Self::MAX_SCROLL - 1)) && !c.is_whitespace())
            .skip(1)
            .find(|&(_n, (_i, c))| !c.is_whitespace())
        {
            self.start_position += i;
        } else {
            self.reset_scroll();
        }
    }
}

impl<T: Display + PartialEq> Widget for ScrollingLabel<T> {
    type Data = T;

    fn event(&mut self, event: &WidgetEvent, data: &Self::Data) {
        match event {
            WidgetEvent::Tick(..) => self.update_scroll(data),
        }
    }

    fn update(&mut self, old_data: &Self::Data, data: &Self::Data) {
        if old_data != data {
            self.force_repaint(data);
        }
    }

    fn force_repaint(&mut self, _data: &Self::Data) {
        self.text = None;
        self.reset_scroll();
    }

    fn paint(&mut self, data: &Self::Data, display: &mut impl TextDisplay) {
        if self.needs_repainting {
            self.needs_repainting = false;

            display.write_to(
                self.segment,
                &Self::generate_text(&mut self.text, data)[self.start_position..],
            );
        }
    }
}

#[derive(Clone, Copy)]
pub enum Either<A, B> {
    A(A),
    B(B),
}

pub trait IntoEither {
    type A;
    type B;

    fn into_either(self) -> Either<Self::A, Self::B>;
}

impl<A, B> IntoEither for Either<A, B> {
    type A = A;
    type B = B;

    fn into_either(self) -> Either<A, B> {
        self
    }
}

impl<T> IntoEither for Option<T> {
    type A = T;
    type B = ();

    fn into_either(self) -> Either<Self::A, Self::B> {
        match self {
            Some(some) => Either::A(some),
            None => Either::B(()),
        }
    }
}

pub struct EitherWidget<T, A, B> {
    a: A,
    b: B,
    _data: PhantomData<fn(&T)>,
}

impl<T, A, B> EitherWidget<T, A, B>
where
    T: Clone + IntoEither,
    A: Widget<Data = <T as IntoEither>::A>,
    B: Widget<Data = <T as IntoEither>::B>,
{
    pub fn new(a: A, b: B) -> Self {
        Self {
            a,
            b,
            _data: PhantomData,
        }
    }
}

impl<T, A, B> Widget for EitherWidget<T, A, B>
where
    T: Clone + IntoEither,
    A: Widget<Data = <T as IntoEither>::A>,
    B: Widget<Data = <T as IntoEither>::B>,
{
    type Data = T;

    fn event(&mut self, event: &WidgetEvent, data: &Self::Data) {
        match data.clone().into_either() {
            Either::A(data) => self.a.event(event, &data),
            Either::B(data) => self.b.event(event, &data),
        }
    }

    fn update(&mut self, old_data: &Self::Data, data: &Self::Data) {
        match (old_data.clone().into_either(), data.clone().into_either()) {
            (Either::A(old_data), Either::A(data)) => self.a.update(&old_data, &data),
            (Either::B(old_data), Either::B(data)) => self.b.update(&old_data, &data),
            (Either::B(_), Either::A(data)) => self.a.force_repaint(&data),
            (Either::A(_), Either::B(data)) => self.b.force_repaint(&data),
        }
    }

    fn force_repaint(&mut self, data: &Self::Data) {
        match data.clone().into_either() {
            Either::A(data) => self.a.force_repaint(&data),
            Either::B(data) => self.b.force_repaint(&data),
        }
    }

    fn paint(&mut self, data: &Self::Data, display: &mut impl TextDisplay) {
        match data.clone().into_either() {
            Either::A(data) => self.a.paint(&data, display),
            Either::B(data) => self.b.paint(&data, display),
        }
    }
}

pub trait Scope {
    type In;
    type Out;

    fn event(&mut self, event: &WidgetEvent, data: &Self::In);
    fn update(&mut self, old_data: &Self::In, data: &Self::In);

    fn reset(&mut self);

    fn data(&self, data: &Self::In) -> Self::Out;
}

pub struct FunctionScope<Data: Clone, In, Out, E, U, D>
where
    E: Fn(&mut Data, &WidgetEvent, &In),
    U: Fn(&mut Data, &In, &In),
    D: Fn(&Data, &In) -> Out,
{
    initial_data: Data,
    data: Data,
    handle_event: E,
    handle_update: U,
    produce_data: D,
    _data: PhantomData<fn(&In) -> Out>,
}

impl<Data: Clone, In, Out, E, U, D> FunctionScope<Data, In, Out, E, U, D>
where
    E: Fn(&mut Data, &WidgetEvent, &In),
    U: Fn(&mut Data, &In, &In),
    D: Fn(&Data, &In) -> Out,
{
    pub fn new(data: Data, handle_event: E, handle_update: U, produce_data: D) -> Self {
        Self {
            initial_data: data.clone(),
            data,
            handle_event,
            handle_update,
            produce_data,
            _data: PhantomData,
        }
    }
}

impl<Data: Clone, In, Out, E, U, D> Scope for FunctionScope<Data, In, Out, E, U, D>
where
    E: Fn(&mut Data, &WidgetEvent, &In),
    U: Fn(&mut Data, &In, &In),
    D: Fn(&Data, &In) -> Out,
{
    type In = In;
    type Out = Out;

    fn event(&mut self, event: &WidgetEvent, data: &Self::In) {
        (self.handle_event)(&mut self.data, event, data)
    }

    fn update(&mut self, old_data: &Self::In, data: &Self::In) {
        (self.handle_update)(&mut self.data, old_data, data)
    }

    fn reset(&mut self) {
        self.data = self.initial_data.clone();
    }

    fn data(&self, data: &Self::In) -> Self::Out {
        (self.produce_data)(&self.data, data)
    }
}

pub struct ScopeWidget<Data, W: Widget, S: Scope<In = Data, Out = W::Data>> {
    inner: W,
    scope: S,
}

impl<Data, W: Widget, S: Scope<In = Data, Out = W::Data>> Widget for ScopeWidget<Data, W, S> {
    type Data = Data;

    fn event(&mut self, event: &WidgetEvent, data: &Self::Data) {
        let old_combined_data = self.scope.data(data);
        self.scope.event(event, data);
        let combined_data = self.scope.data(data);

        self.inner.update(&old_combined_data, &combined_data);

        self.inner.event(event, &combined_data);
    }

    fn update(&mut self, old_data: &Self::Data, data: &Self::Data) {
        let old_combined_data = self.scope.data(old_data);
        self.scope.update(old_data, data);
        let combined_data = self.scope.data(data);

        self.inner.update(&old_combined_data, &combined_data);
    }

    fn force_repaint(&mut self, data: &Self::Data) {
        self.scope.reset();
        self.inner.force_repaint(&self.scope.data(data))
    }

    fn paint(&mut self, data: &Self::Data, display: &mut impl TextDisplay) {
        self.inner.paint(&self.scope.data(data), display)
    }
}

pub struct LensWidget<Data, W: Widget, L: Fn(&Data) -> W::Data> {
    lens: L,
    inner: W,
    _phantom_data: PhantomData<fn(&Data)>,
}

impl<Data, W: Widget, L: Fn(&Data) -> W::Data> Widget for LensWidget<Data, W, L> {
    type Data = Data;

    fn event(&mut self, event: &WidgetEvent, data: &Data) {
        self.inner.event(event, &(self.lens)(data))
    }

    fn update(&mut self, old_data: &Data, data: &Data) {
        self.inner
            .update(&(self.lens)(old_data), &(self.lens)(data))
    }

    fn force_repaint(&mut self, data: &Data) {
        self.inner.force_repaint(&(self.lens)(data))
    }

    fn paint(&mut self, data: &Data, display: &mut impl TextDisplay) {
        self.inner.paint(&(self.lens)(data), display)
    }
}

pub struct WidgetGroup<T, W1, W2>(W1, W2, PhantomData<fn(&T)>);

impl<T, W1: Widget<Data = T>, W2: Widget<Data = T>> Widget for WidgetGroup<T, W1, W2> {
    type Data = T;

    fn event(&mut self, event: &WidgetEvent, data: &Self::Data) {
        self.0.event(event, data);
        self.1.event(event, data);
    }

    fn update(&mut self, old_data: &Self::Data, data: &Self::Data) {
        self.0.update(old_data, data);
        self.1.update(old_data, data);
    }

    fn force_repaint(&mut self, data: &Self::Data) {
        self.0.force_repaint(data);
        self.1.force_repaint(data);
    }

    fn paint(&mut self, data: &Self::Data, display: &mut impl TextDisplay) {
        self.0.paint(data, display);
        self.1.paint(data, display);
    }
}

pub trait WidgetExt: Widget {
    /// Wrap this widget in a [LensWidget] widget for the provided lens
    fn with_lens<Data, M: Fn(&Data) -> Self::Data>(self, lens: M) -> LensWidget<Data, Self, M> {
        LensWidget {
            lens,
            inner: self,
            _phantom_data: PhantomData,
        }
    }

    /// Wrap this widget in a [ScopeWidget] widget with the provided scope
    fn with_scope<S: Scope<Out = Self::Data>>(self, scope: S) -> ScopeWidget<S::In, Self, S> {
        ScopeWidget { inner: self, scope }
    }

    /// Group this widget with the provided widget
    fn group<W: Widget<Data = Self::Data>>(self, widget: W) -> WidgetGroup<Self::Data, Self, W> {
        WidgetGroup(self, widget, PhantomData)
    }
}

impl<W: Widget> WidgetExt for W {}

pub struct PassThrough<W>(pub W);

impl<T, W: Widget<Data = T>> Widget for PassThrough<W> {
    type Data = T;

    fn event(&mut self, event: &WidgetEvent, data: &Self::Data) {
        self.0.event(event, data)
    }

    fn update(&mut self, old_data: &Self::Data, data: &Self::Data) {
        self.0.update(old_data, data)
    }

    fn force_repaint(&mut self, data: &Self::Data) {
        self.0.force_repaint(data)
    }

    fn paint(&mut self, data: &Self::Data, display: &mut impl TextDisplay) {
        self.0.paint(data, display)
    }
}
