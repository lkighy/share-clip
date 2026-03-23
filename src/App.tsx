import React from "react";
import ReactDOM from "react-dom/client";
import { BrowserRouter, Routes, Route } from 'react-router-dom';
import ClipboardWindow from "@/pages/ClipboardWindow.tsx";
import ShareFilesWindow from "@/pages/ShareFilesWindow.tsx";
import AppConfigWindow from "@/pages/AppConfigWindow.tsx";
import "./index.css";

ReactDOM.createRoot(document.getElementById('root')!).render(
    <React.StrictMode>
        <BrowserRouter>
            <Routes>
                <Route path="/" element={<ClipboardWindow />} />
                <Route path="/clipboard" element={<ClipboardWindow />} />
                <Route path="/shared-files" element={<ShareFilesWindow />} />
                <Route path="/app-config" element={<AppConfigWindow />} />
            </Routes>
        </BrowserRouter>
    </React.StrictMode>
);