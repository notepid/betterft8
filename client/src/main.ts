import './app.css'
import { mount } from 'svelte'
import App from './App.svelte'
// Importing the theme store runs its apply-subscriber, which sets the persisted
// `data-theme` on the document root before mount — avoids a flash of the default.
import { theme } from './lib/stores'

void theme

mount(App, { target: document.body })
