# ThermalPrint design

## Overview
ThermalPrint is a Rust egui desktop workspace for preparing physical labels. Preserve its Voltkit-inspired full sidebar, spacious canvas, quiet panels, and system light/dark appearance. New tools belong beside existing printer, history, and folder controls.

## Colors
Runtime colors belong to `ThermalPrint::new`, `section_frame`, and shared controls in `src/ui`. Selection uses RGB 46, 101, 190. Teal identifies workspace actions. Error text and disabled states follow egui visuals. Never encode status solely by color.

## Typography
Bundled font setup in `src/fonts.rs` owns typography. Body is 14 points, headings 23, small text 12. Preserve English, Spanish, German, and French text routing through `i18n::tr`.

## Layout
The native window has a 760 by 560 minimum. Print Center adapts between one and two columns. Long tools scroll within the existing content surface.

## Components
Reuse egui buttons, ComboBox, Window, ScrollArea and the shared notices system. Settings persist through `Settings`; rendering and printer submission stay in their canonical modules.
