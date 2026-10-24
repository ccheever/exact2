// Each specimen of specimens.json as WinUI builds it, with the words Shelf's
// screens use. The window's theme is the process's (App.cs), so a control
// here takes its colours from Fluent's theme resources as any WinUI app does.
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Markup;

namespace Reference;

static class Specimens
{
    public static FrameworkElement Build(string id) => id switch
    {
        "text-field-empty" => new TextBox { PlaceholderText = "Title" },
        "text-field-filled" => new TextBox { Text = "Piranesi" },
        "text-field-disabled" => new TextBox { Text = "Piranesi", IsEnabled = false },
        // App.cs gives it keyboard focus once the window is shown.
        "text-field-focused" => new TextBox { PlaceholderText = "Title" },
        "number-field" => new NumberBox { Value = 272 },
        "search" => new AutoSuggestBox
        {
            PlaceholderText = "Search",
            QueryIcon = new SymbolIcon(Symbol.Find),
        },
        "button" => new Button { Content = "Add Book" },
        "button-accent" => new Button
        {
            Content = "Save",
            Style = (Style)Application.Current.Resources["AccentButtonStyle"],
        },
        "button-disabled" => new Button { Content = "Add Book", IsEnabled = false },
        "segmented" => Segmented(),
        "list-row" => ListRow(),
        "progress" => new ProgressBar { Value = 44 },
        "navigation-pane" => NavigationPane(),
        _ => throw new System.ArgumentException($"no specimen {id}"),
    };

    static SelectorBar Segmented()
    {
        var bar = new SelectorBar();
        foreach (var text in new[] { "All", "Reading", "Want", "Finished" })
        {
            bar.Items.Add(new SelectorBarItem { Text = text });
        }
        bar.SelectedItem = bar.Items[0];
        return bar;
    }

    // A two-line row in a ListView, as a list of books draws one.
    static ListView ListRow()
    {
        var list = new ListView
        {
            SelectionMode = ListViewSelectionMode.None,
            ItemTemplate = (DataTemplate)XamlReader.Load(
                "<DataTemplate xmlns='http://schemas.microsoft.com/winfx/2006/xaml/presentation'>"
                + "<StackPanel Padding='0,8'>"
                + "<TextBlock Text='Piranesi' Style='{StaticResource BodyStrongTextBlockStyle}'/>"
                + "<TextBlock Text='Susanna Clarke' Style='{StaticResource CaptionTextBlockStyle}'"
                + " Foreground='{ThemeResource TextFillColorSecondaryBrush}'/>"
                + "</StackPanel></DataTemplate>"),
        };
        list.Items.Add("seed-2");
        return list;
    }

    // The left pane an unsaid root tab bar becomes (D14), held expanded.
    static NavigationView NavigationPane()
    {
        var nav = new NavigationView
        {
            PaneDisplayMode = NavigationViewPaneDisplayMode.Left,
            IsBackButtonVisible = NavigationViewBackButtonVisible.Collapsed,
            IsSettingsVisible = false,
            IsPaneToggleButtonVisible = true,
        };
        foreach (var (text, symbol) in new[]
        {
            ("Library", Symbol.Library),
            ("Discover", Symbol.World),
            ("Stats", Symbol.ViewAll),
        })
        {
            nav.MenuItems.Add(new NavigationViewItem { Content = text, Icon = new SymbolIcon(symbol) });
        }
        nav.SelectedItem = nav.MenuItems[0];
        nav.Content = new TextBlock
        {
            Text = "Library",
            Margin = new Thickness(24, 16, 0, 0),
            Style = (Style)Application.Current.Resources["TitleTextBlockStyle"],
        };
        return nav;
    }
}
