import React from 'react';
import {createRoot} from 'react-dom/client';
import 'c2pa-react-component/style.css';
import 'c2pa-react-ai-disclaimer/style.css';
import './style.css';
import {App} from './App.jsx';

createRoot(document.getElementById('root')).render(<App/>);
