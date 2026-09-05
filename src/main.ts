import { mount } from 'svelte';
import App from './ui/App.svelte';
import './ui/style.css';

mount(App, { target: document.getElementById('app')! });
