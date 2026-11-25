//! The rules of group K: concurrency.

mod common;
use common::{fixed, lines_of, messages, unsafe_fixed};

#[test]
fn thread_run() {
    let source = r#"import java.util.concurrent.locks.*;
class A {
    Thread field;
    void f(Thread parameter, Runnable r, Object o) {
        new Thread(r).run();
        parameter.run();
        field.run();
        this.field.run();
        Thread.currentThread().run();
        ((Thread) o).run();
        var t = new Thread(r);
        t.run();
        java.lang.Thread q = null;
        q.run();
        r.run();
        Thread[] ts = null;
        ts[0].run();
        parameter.run(1);
        super.run();
        o.run();
    }
    void g(Thread... ts) { ts.run(); }
    void h(Runnable parameter) { parameter.run(); }
}
class B extends Object {
    void f() { inherited.run(); }
}
"#;
    assert_eq!(lines_of("K001", source), [5, 6, 7, 8, 9, 10, 12, 14]);
    assert_eq!(messages("K001", "class A { void f(Thread t) { t.run(); } }"), ["Thread.run() instead of start()"]);
    assert_eq!(fixed("K001", "class A { void f(Thread t) { t.run(); } }"), "class A { void f(Thread t) { t.run(); } }");
    assert_eq!(
        unsafe_fixed("K001", "class A { void f(Thread t) { t . run(); } }"),
        "class A { void f(Thread t) { t . start(); } }"
    );
}

#[test]
fn thread_run_shadowed_types() {
    // A project-local Thread class, and an import of another one.
    let local = "class A { void f(Thread t) { t.run(); } } class Thread { void run() {} }";
    assert!(lines_of("K001", local).is_empty());
    let imported = "import org.example.Thread;\nclass A { void f(Thread t) { t.run(); } }";
    assert!(lines_of("K001", imported).is_empty());
    let generic = "class A<Thread> { void f(Thread t) { t.run(); } }";
    assert!(lines_of("K001", generic).is_empty());
    // A local variable shadows the field, a lambda parameter without type is unknown.
    let shadowed = r#"class A {
    Thread t;
    void f(Runnable t) { t.run(); }
    void g() { java.util.function.Consumer<Runnable> c = t -> t.run(); }
}
"#;
    assert!(lines_of("K001", shadowed).is_empty());
}

#[test]
fn thread_not_started() {
    let source = r#"class A {
    void f(Runnable r) {
        new Thread(r);
        new Thread(() -> {});
        new java.lang.Thread(r);
        new Thread() { public void run() {} };
        Thread t = new Thread(r);
        new Thread(r).start();
        new Object();
        new MyThread();
    }
}
"#;
    assert_eq!(lines_of("K002", source), [3, 4, 5, 6]);
    assert_eq!(messages("K002", "class A { void f() { new Thread(); } }"), ["Thread created but never started"]);
}

#[test]
fn thread_stop() {
    let source = r#"class A {
    void f(Thread t, Object o) {
        t.stop();
        t.suspend();
        t.resume();
        Thread.currentThread().stop();
        o.stop();
        t.stop(new Error());
        t.interrupt();
    }
}
"#;
    assert_eq!(lines_of("K006", source), [3, 4, 5, 6]);
    assert_eq!(messages("K006", "class A { void f(Thread t) { t.suspend(); } }"), ["Thread.suspend() is unsafe"]);
}

#[test]
fn interrupted_in_catch() {
    let source = r#"class A {
    void f() {
        try {
            Thread.sleep(1);
        } catch (InterruptedException e) {
            Thread.interrupted();
            if (Thread.interrupted()) {}
            Runnable r = () -> Thread.interrupted();
        } catch (RuntimeException e) {
            Thread.interrupted();
        }
        try {
            g();
        } catch (InterruptedException | java.io.IOException e) {
            try {
                g();
            } catch (InterruptedException e2) {
                Thread.interrupted();
            }
        }
        Thread.interrupted();
    }
}
"#;
    assert_eq!(lines_of("K010", source), [6, 7, 18]);
    let fix = r#"class A {
    void f() {
        try {
            Thread.sleep(1);
        } catch (InterruptedException e) {
            Thread.interrupted();
            boolean b = Thread.interrupted();
            Thread./* c */interrupted();
        }
    }
}
"#;
    assert_eq!(
        fixed("K010", fix),
        r#"class A {
    void f() {
        try {
            Thread.sleep(1);
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
            boolean b = Thread.interrupted();
            Thread./* c */interrupted();
        }
    }
}
"#
    );
    let shadowed = r#"import org.example.InterruptedException;
class A { void f() { try { g(); } catch (InterruptedException e) { Thread.interrupted(); } } }
"#;
    assert!(lines_of("K010", shadowed).is_empty());
}

#[test]
fn wait_not_in_loop() {
    let source = r#"import java.util.concurrent.locks.Condition;
import java.util.concurrent.CountDownLatch;
class A {
    Condition condition;
    void f(Object monitor, CountDownLatch latch, boolean ready) throws Exception {
        if (!ready) monitor.wait();
        wait();
        wait(100);
        condition.await();
        condition.awaitNanos(5);
        latch.await();
        while (!ready) monitor.wait();
        do { wait(); } while (!ready);
        for (;;) { if (!ready) condition.await(); }
        while (!ready) { Runnable r = () -> { try { wait(); } catch (Exception e) {} }; }
    }
}
"#;
    assert_eq!(lines_of("K016", source), [6, 7, 8, 9, 10, 15]);
    assert_eq!(messages("K016", "class A { void f() throws Exception { wait(); } }"), ["wait() not called in a loop"]);
    // A method named wait may be an overload.
    let overload = "class A { void wait(String s) {} void f() { wait(\"x\"); } }";
    assert!(lines_of("K016", overload).is_empty());
    // A project-local Condition.
    let local = "class A { void f(Condition c) { c.await(); } }";
    assert!(lines_of("K016", local).is_empty());
}

#[test]
fn wait_on_condition() {
    let source = r#"import java.util.concurrent.locks.*;
class A {
    void f(Lock lock, Object o) throws Exception {
        Condition c = lock.newCondition();
        c.wait();
        c.wait(100);
        c.notify();
        c.notifyAll();
        c.await();
        o.wait();
    }
}
"#;
    assert_eq!(lines_of("K023", source), [5, 6, 7, 8]);
    assert_eq!(
        messages(
            "K023",
            "import java.util.concurrent.locks.Condition; class A { void f(Condition c) { c.notify(); } }"
        ),
        ["notify() on a Condition instead of signal()"]
    );
    assert_eq!(
        unsafe_fixed("K023", source),
        source
            .replace("c.wait();", "c.await();")
            .replace("c.notify();", "c.signal();")
            .replace("c.notifyAll();", "c.signalAll();")
    );
}

#[test]
fn lock_without_try() {
    let source = r#"import java.util.concurrent.locks.*;
class A {
    private final Lock lock = new ReentrantLock();
    private final ReentrantReadWriteLock rw = new ReentrantReadWriteLock();
    void ok() {
        lock.lock();
        try {
            work();
        } finally {
            lock.unlock();
        }
    }
    void unlockNotInFinally() {
        lock.lock();
        work();
        lock.unlock();
    }
    void statementBeforeTry() {
        lock.lock();
        work();
        try {
            work();
        } finally {
            lock.unlock();
        }
    }
    void insideTry() {
        try {
            lock.lock();
            work();
        } finally {
            lock.unlock();
        }
    }
    void readLock() {
        rw.readLock().lock();
        work();
        rw.readLock().unlock();
    }
    void releasedElsewhere() {
        lock.lock();
        work();
    }
    void twoLocks(Lock other) {
        lock.lock();
        other.lock();
        try {
            work();
        } finally {
            other.unlock();
            lock.unlock();
        }
    }
    void unknownType(MyLock mine) {
        mine.lock();
        work();
        mine.unlock();
    }
}
"#;
    assert_eq!(lines_of("K024", source), [14, 19, 29, 36]);
    assert_eq!(
        messages("K024", source)[..3],
        [
            "lock() not directly followed by a try block that releases the lock in finally",
            "lock() not directly followed by a try block that releases the lock in finally",
            "lock() inside the try block that releases the lock",
        ]
    );
}

#[test]
fn synchronize_on_non_final() {
    let source = r#"class A {
    private Object lock = new Object();
    private final Object finalLock = new Object();
    private static Object staticLock = new Object();
    @LazyInit private Object lazy;
    void f(Object parameter) {
        synchronized (lock) {}
        synchronized (this.lock) {}
        synchronized (staticLock) {}
        synchronized (finalLock) {}
        synchronized (parameter) {}
        synchronized (lazy) {}
        Object local = new Object();
        synchronized (local) {}
        synchronized (this) {}
    }
}
interface I {
    Object LOCK = new Object();
    default void f() { synchronized (LOCK) {} }
}
class B extends Base {
    void f() { synchronized (inherited) {} }
}
record R(Object lock) {
    void f() { synchronized (lock) {} }
}
"#;
    assert_eq!(lines_of("K026", source), [7, 8, 9]);
    assert_eq!(
        messages("K026", "class A { Object lock; void f() { synchronized (lock) {} } }"),
        ["Synchronization on non-final field 'lock'"]
    );
}

#[test]
fn synchronize_on_value_based() {
    let source = r#"import java.time.*;
class A {
    private final Integer count = 0;
    private final Object lock = new Object();
    void f(Boolean flag, LocalDate date, String s) {
        synchronized (count) {}
        synchronized (flag) {}
        synchronized (date) {}
        synchronized (lock) {}
        synchronized (s) {}
        java.util.Optional<String> o = java.util.Optional.empty();
        synchronized (o) {}
    }
}
"#;
    assert_eq!(lines_of("K027", source), [6, 7, 8, 12]);
}

#[test]
fn static_field_instance_lock() {
    let source = r#"class A {
    static int count;
    static int other;
    int instance;
    final Object lock = new Object();
    synchronized void f() {
        count++;
        A.other = 1;
        instance++;
        int count = 0;
        count++;
    }
    void g() {
        synchronized (this) {
            other += 2;
        }
        synchronized (lock) {
            count--;
        }
        synchronized (A.class) {
            count++;
            synchronized (this) {
                count++;
            }
        }
        count++;
    }
    static synchronized void h() {
        count++;
    }
    synchronized void i() {
        synchronized (this) {
            count++;
        }
        Runnable r = () -> count++;
    }
}
"#;
    assert_eq!(lines_of("K042", source), [7, 8, 15, 18, 33]);
    assert_eq!(messages("K042", source)[0], "Static field 'count' written under an instance lock");
}

#[test]
fn non_atomic_volatile() {
    let source = r#"class A {
    volatile int n;
    volatile boolean flag;
    int plain;
    void f(int x) {
        n++;
        --n;
        n += 1;
        this.n -= x;
        n = n + 1;
        n = 2 * (n);
        flag |= true;
        n = x + 1;
        n = 5;
        plain++;
        int local = 0;
        local++;
        synchronized (this) { n++; }
    }
    synchronized void g() { n++; }
    void h(int n) { n++; }
}
"#;
    assert_eq!(lines_of("K043", source), [6, 7, 8, 9, 10, 11, 12]);
    assert_eq!(
        messages("K043", "class A { volatile int n; void f() { n++; } }"),
        ["Non-atomic update of volatile field 'n'"]
    );
}

#[test]
fn static_formatter() {
    let source = r#"import java.text.*;
import java.text.SimpleDateFormat;
class A {
    static final SimpleDateFormat F = new SimpleDateFormat("yyyy");
    private static DateFormat format;
    static NumberFormat a, b;
    static final java.text.MessageFormat M = null;
    final SimpleDateFormat instance = null;
    static ThreadLocal<SimpleDateFormat> local;
    static SimpleDateFormat[] array;
    void f() { SimpleDateFormat local = null; }
}
interface I {
    DecimalFormat D = new DecimalFormat();
}
"#;
    assert_eq!(lines_of("K047", source), [4, 5, 6, 6, 7, 14]);
    // Without the import, the name is another class.
    assert!(lines_of("K047", "class A { static SimpleDateFormat F; }").is_empty());
    assert_eq!(
        messages("K047", "import java.text.SimpleDateFormat; class A { static SimpleDateFormat F; }"),
        ["SimpleDateFormat is not thread-safe but held in a static field"]
    );
}

#[test]
fn zero_core_pool() {
    let source = r#"import java.util.concurrent.*;
class A {
    Object a = new ScheduledThreadPoolExecutor(0);
    Object b = new ScheduledThreadPoolExecutor(0, Executors.defaultThreadFactory());
    Object c = Executors.newScheduledThreadPool(0);
    Object d = new ScheduledThreadPoolExecutor(1);
    Object e = Executors.newScheduledThreadPool(1);
    Object f = new ThreadPoolExecutor(0, 1, 1, TimeUnit.SECONDS, null);
}
"#;
    assert_eq!(lines_of("K058", source), [3, 4, 5]);
    assert!(lines_of("K058", "class A { Object a = new ScheduledThreadPoolExecutor(0); }").is_empty());
}
