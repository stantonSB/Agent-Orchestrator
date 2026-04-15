import styles from "./App.module.css";
import { TitleBar, TerminalArea, SessionPanel } from "./components";

function App() {
  return (
    <div className={styles.app}>
      <TitleBar />
      <div className={styles.mainContent}>
        <TerminalArea activeSessionId={null} />
        <SessionPanel sessionCount={0} />
      </div>
    </div>
  );
}

export default App;
