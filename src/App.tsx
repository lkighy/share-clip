import React, { Suspense, lazy } from "react";
import ReactDOM from "react-dom/client";
import { BrowserRouter, Routes, Route } from 'react-router-dom';
import { installFrontendLogger } from "@/lib/logger";
import { initThemeMode, loadAppConfig } from "@/store/appConfigStore";
import "./index.css";

const ClipboardWindow = lazy(() => import("@/pages/ClipboardWindow.tsx"));
const ShareFilesWindow = lazy(() => import("@/pages/ShareFilesWindow.tsx"));
const AppConfigWindow = lazy(() => import("@/pages/AppConfigWindow.tsx"));

installFrontendLogger();
initThemeMode();
void loadAppConfig();

ReactDOM.createRoot(document.getElementById('root')!).render(
    <React.StrictMode>
        <BrowserRouter>
            <Suspense fallback={null}>
                <Routes>
                    <Route path="/" element={<ClipboardWindow />} />
                    <Route path="/clipboard" element={<ClipboardWindow />} />
                    <Route path="/shared-files" element={<ShareFilesWindow />} />
                    <Route path="/app-config" element={<AppConfigWindow />} />
                </Routes>
            </Suspense>
        </BrowserRouter>
    </React.StrictMode>
);
